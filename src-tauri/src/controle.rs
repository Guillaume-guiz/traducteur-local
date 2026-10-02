//! Dictée à la touche Contrôle seule : on maintient Contrôle, on parle, on
//! relâche. Un raccourci classique ne sait pas écouter une touche de
//! modification seule ; on observe donc le clavier au niveau du système
//! (« event tap » actif), ce qui demande l'autorisation Accessibilité, déjà
//! nécessaire pour coller. On ne lit jamais les touches tapées : seulement
//! « Contrôle enfoncé ou relâché » et « une autre touche a servi pendant ce temps ».
//!
//! Une fois la dictée lancée, un clic ne l'annule plus (30/09/2026, retour de
//! Guillaume) : il sert à placer le curseur là où le texte sera collé. On retire
//! Contrôle de ce clic, sinon macOS en ferait un clic droit (menu contextuel).
//! Avant le lancement (les 200 premières ms), Contrôle + clic reste un clic droit.

use core_foundation::base::TCFType;
use core_foundation::mach_port::CFMachPortRef;
use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
use core_graphics::event::{
    CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventType, CallbackResult,
};
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;
use tauri::AppHandle;

/// Délai avant d'ouvrir le micro : un Contrôle tenu moins longtemps sert à un
/// raccourci (⌃C, ⌃A...) et ne doit rien déclencher.
const HOLD_DELAY_MS: u64 = 200; // 0,3 s jusqu'au 27/09/2026, 0,2 s au choix de Guillaume

/// Port du tap, pour le réactiver si macOS le coupe (callback trop lent).
static TAP_PORT: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());
/// La dictée se fait à la touche Contrôle seule (réglable depuis le panneau).
static MODE: AtomicBool = AtomicBool::new(false);
/// Écoute démarrée (une seule fois ; ensuite MODE l'active ou non).
static DEMARREE: AtomicBool = AtomicBool::new(false);
/// Vrai pendant qu'on tape un nouveau raccourci : Contrôle ne dicte pas.
static SUSPENDUE: AtomicBool = AtomicBool::new(false);

/// Passe la dictée à la touche Contrôle seule (`oui`) ou la rend à un
/// raccourci classique ; démarre l'écoute du clavier la première fois.
pub fn activer(app: &AppHandle, oui: bool) {
    MODE.store(oui, Ordering::SeqCst);
    if oui && !DEMARREE.swap(true, Ordering::SeqCst) {
        start(app);
    }
}

pub fn suspendre(oui: bool) {
    SUSPENDUE.store(oui, Ordering::SeqCst);
}

fn ecoute_active() -> bool {
    MODE.load(Ordering::SeqCst) && !SUSPENDUE.load(Ordering::SeqCst)
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
}

enum Signal {
    Down(u64),
    Up(u64),
    Cancel(u64),
}

struct Shared {
    ctrl_down: AtomicBool,
    /// Une autre touche, un autre modificateur ou un clic a servi pendant que
    /// Contrôle était tenu : c'était un raccourci, pas une dictée.
    other_used: AtomicBool,
    /// Numéro de l'appui en cours sur Contrôle.
    gen: AtomicU64,
    /// Numéro de l'appui pour lequel le micro a été ouvert (0 = aucun).
    recording_gen: AtomicU64,
    /// Numéro de l'appui reconnu comme une dictée (posé avant l'ouverture du
    /// micro, qui peut prendre plusieurs secondes avec un casque Bluetooth).
    en_dictee: AtomicU64,
}

/// Démarre l'écoute de la touche Contrôle (attend l'autorisation Accessibilité
/// si elle n'est pas encore donnée, sans relancer l'app).
fn start(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let mut prompted = false;
        while !crate::accessibility_trusted() {
            if !prompted {
                crate::dictee::log_line(&app, "attente_accessibilite");
                crate::request_accessibility_prompt();
                prompted = true;
            }
            std::thread::sleep(Duration::from_secs(2));
        }

        let sh = Arc::new(Shared {
            ctrl_down: AtomicBool::new(false),
            other_used: AtomicBool::new(false),
            gen: AtomicU64::new(0),
            recording_gen: AtomicU64::new(0),
            en_dictee: AtomicU64::new(0),
        });
        let (tx, rx) = mpsc::channel::<Signal>();
        spawn_worker(app.clone(), sh.clone(), rx);

        // Le callback doit rester instantané : il ne fait que noter l'état et
        // passer la main au fil de travail.
        let cb_sh = sh.clone();
        let tap = CGEventTap::new(
            CGEventTapLocation::Session,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::Default,
            vec![
                CGEventType::FlagsChanged,
                CGEventType::KeyDown,
                CGEventType::LeftMouseDown,
                CGEventType::LeftMouseUp,
                CGEventType::LeftMouseDragged,
                CGEventType::RightMouseDown,
                CGEventType::RightMouseUp,
                CGEventType::RightMouseDragged,
                CGEventType::OtherMouseDown,
                CGEventType::OtherMouseUp,
                CGEventType::OtherMouseDragged,
                CGEventType::ScrollWheel,
            ],
            move |_proxy, etype, event| {
                let sh = &cb_sh;
                match etype {
                    CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput => {
                        let p = TAP_PORT.load(Ordering::SeqCst);
                        if !p.is_null() {
                            unsafe { CGEventTapEnable(p as CFMachPortRef, true) };
                        }
                    }
                    CGEventType::FlagsChanged => {
                        let flags = event.get_flags();
                        let ctrl = flags.contains(CGEventFlags::CGEventFlagControl);
                        let others = flags.intersects(
                            CGEventFlags::CGEventFlagCommand
                                | CGEventFlags::CGEventFlagAlternate
                                | CGEventFlags::CGEventFlagShift,
                        );
                        let was = sh.ctrl_down.load(Ordering::SeqCst);
                        if ctrl && !was {
                            sh.ctrl_down.store(true, Ordering::SeqCst);
                            let g = sh.gen.fetch_add(1, Ordering::SeqCst) + 1;
                            sh.other_used.store(others, Ordering::SeqCst);
                            if !others {
                                let _ = tx.send(Signal::Down(g));
                            }
                        } else if !ctrl && was {
                            sh.ctrl_down.store(false, Ordering::SeqCst);
                            let _ = tx.send(Signal::Up(sh.gen.load(Ordering::SeqCst)));
                        } else if ctrl && others && !sh.other_used.swap(true, Ordering::SeqCst) {
                            let _ = tx.send(Signal::Cancel(sh.gen.load(Ordering::SeqCst)));
                        }
                    }
                    CGEventType::KeyDown => {
                        // Touche tapée pendant que Contrôle est tenu : un raccourci (⌃C...).
                        if sh.ctrl_down.load(Ordering::SeqCst)
                            && !sh.other_used.swap(true, Ordering::SeqCst)
                        {
                            let _ = tx.send(Signal::Cancel(sh.gen.load(Ordering::SeqCst)));
                        }
                    }
                    _ => {
                        // Clic, glisser ou défilement.
                        let ctrl = sh.ctrl_down.load(Ordering::SeqCst);
                        let en_dictee = ctrl
                            && sh.en_dictee.load(Ordering::SeqCst) == sh.gen.load(Ordering::SeqCst);
                        if en_dictee {
                            // Dictée en cours : clic normal, pour placer le curseur.
                            let flags = event.get_flags();
                            if flags.contains(CGEventFlags::CGEventFlagControl) {
                                event.set_flags(flags & !CGEventFlags::CGEventFlagControl);
                            }
                        } else if ctrl
                            && matches!(
                                etype,
                                CGEventType::LeftMouseDown
                                    | CGEventType::RightMouseDown
                                    | CGEventType::OtherMouseDown
                                    | CGEventType::ScrollWheel
                            )
                            && !sh.other_used.swap(true, Ordering::SeqCst)
                        {
                            // Avant la dictée : Contrôle + clic est un clic droit.
                            let _ = tx.send(Signal::Cancel(sh.gen.load(Ordering::SeqCst)));
                        }
                    }
                }
                CallbackResult::Keep
            },
        );
        match tap {
            Ok(tap) => {
                TAP_PORT.store(
                    tap.mach_port().as_concrete_TypeRef() as *mut std::ffi::c_void,
                    Ordering::SeqCst,
                );
                let source = match tap.mach_port().create_runloop_source(0) {
                    Ok(s) => s,
                    Err(_) => {
                        crate::dictee::log_line(&app, "ecoute_controle echec_source");
                        return;
                    }
                };
                CFRunLoop::get_current().add_source(&source, unsafe { kCFRunLoopCommonModes });
                tap.enable();
                crate::dictee::log_line(&app, "ecoute_controle ok");
                CFRunLoop::run_current();
            }
            Err(()) => crate::dictee::log_line(&app, "ecoute_controle refusee"),
        }
    });
}

/// Traduit les signaux du clavier en début, fin ou abandon de dictée.
fn spawn_worker(app: AppHandle, sh: Arc<Shared>, rx: mpsc::Receiver<Signal>) {
    std::thread::spawn(move || {
        for sig in rx {
            match sig {
                Signal::Down(g) => {
                    if !ecoute_active() {
                        continue;
                    }
                    let sh = sh.clone();
                    let app = app.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(HOLD_DELAY_MS));
                        let still_alone = sh.gen.load(Ordering::SeqCst) == g
                            && sh.ctrl_down.load(Ordering::SeqCst)
                            && !sh.other_used.load(Ordering::SeqCst)
                            && ecoute_active();
                        if !still_alone {
                            return;
                        }
                        // Dès maintenant, un clic place le curseur au lieu d'annuler.
                        sh.en_dictee.store(g, Ordering::SeqCst);
                        // Micro déjà ouvert : on reprend l'audio depuis l'appui sur
                        // Contrôle (délai de vérification + marge), rien n'est perdu.
                        crate::dictee::on_press(&app, HOLD_DELAY_MS + 100);
                        // L'enregistrement n'est déclaré qu'une fois le micro ouvert :
                        // à partir de là, soit le signal de relâchement le termine,
                        // soit on constate ici qu'il est déjà passé. Les deux passent
                        // par compare_exchange, donc un seul des deux agit.
                        sh.recording_gen.store(g, Ordering::SeqCst);
                        let released = !sh.ctrl_down.load(Ordering::SeqCst)
                            || sh.gen.load(Ordering::SeqCst) != g;
                        let cancelled = sh.other_used.load(Ordering::SeqCst);
                        if (released || cancelled)
                            && sh
                                .recording_gen
                                .compare_exchange(g, 0, Ordering::SeqCst, Ordering::SeqCst)
                                .is_ok()
                        {
                            if cancelled {
                                crate::dictee::abort(&app);
                            } else {
                                crate::dictee::release_now(&app);
                            }
                        }
                    });
                }
                Signal::Up(g) => {
                    if sh
                        .recording_gen
                        .compare_exchange(g, 0, Ordering::SeqCst, Ordering::SeqCst)
                        .is_ok()
                    {
                        let app = app.clone();
                        std::thread::spawn(move || crate::dictee::release_now(&app));
                    }
                }
                Signal::Cancel(g) => {
                    if sh
                        .recording_gen
                        .compare_exchange(g, 0, Ordering::SeqCst, Ordering::SeqCst)
                        .is_ok()
                    {
                        let app = app.clone();
                        std::thread::spawn(move || crate::dictee::abort(&app));
                    }
                }
            }
        }
    });
}
