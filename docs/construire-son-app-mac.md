# Construire sa propre app Mac avec Claude Code

Ce dépôt sert de cas réel pour un dojo. L'idée : on peut aujourd'hui construire ses propres outils sans être développeur, à condition de savoir par quelles étapes passer et quoi vérifier. Ce guide les donne, avec à chaque fois ce qui s'est vraiment passé sur le Traducteur local.

## L'histoire en quatre dates

- **16 juin 2026** : première version, la fenêtre de traduction façon DeepL, en local avec Ollama.
- **27 septembre** : la dictée (maintenir Ctrl, parler, relâcher), validée le jour même avec le vrai micro.
- **29 et 30 septembre** : la pastille d'état, le son baissé, les raccourcis réglables, la transcription par morceaux. Tout vient de l'usage quotidien.
- **2 octobre** : version publique figée, celle de ce dépôt.

Le code a été écrit par Claude Code. Les décisions, les essais et les retours d'usage, par une personne qui n'est pas développeur.

## Les dix étapes

### 1. Partir d'un usage précis et d'une contrainte qui tranche

Ce qu'on fait : décrire en quelques phrases ce que l'outil doit faire pour soi, au quotidien. Puis poser la contrainte qui élimine des options.

Le garde-fou : si on ne sait pas dire à quel moment de sa journée on s'en sert, c'est trop tôt pour coder.

Dans le traducteur : « comme DeepL, mais rien ne sort de ma machine ». Cette seule contrainte a imposé un modèle d'IA local (Ollama) et, plus tard, un moteur de transcription embarqué dans l'app.

### 2. Choisir la technique avec Claude, et noter pourquoi

Ce qu'on fait : demander à Claude deux ou trois façons de construire l'app, avec leurs avantages et leurs coûts. Pour une app Mac : Swift (natif, Apple seulement), Tauri (moteur en Rust, interface en HTML), Electron (lourd mais très répandu).

Le garde-fou : faire écrire le choix et sa raison dans le fichier de contexte du projet (étape 4). Sinon, trois semaines plus tard, une nouvelle session de Claude repose la question ou défait le choix.

Dans le traducteur : Tauri, pour une app légère avec une interface simple à modifier. Le raccourci de traduction est un raccourci dédié plutôt que le double Cmd+C de DeepL, pour éviter une autorisation macOS de plus. Ce genre de choix produit se décide au début.

### 3. Préparer le Mac

Ce qu'on fait : installer les outils une fois pour toutes (outils de développement Apple, Rust, Node.js, Homebrew), en demandant à Claude la commande exacte et la version minimale.

Les garde-fous :
- **La place disque.** Une construction d'app Rust occupe plusieurs Go, un modèle d'IA de 4 à 20 Go. Une construction du traducteur a échoué sur un disque plein. Vérifier avant tout gros téléchargement.
- **La synchronisation.** Un projet dans iCloud Drive voit ses milliers de petits fichiers techniques « évincés » du disque : le démarrage prend alors plusieurs minutes. Garder le code hors d'iCloud, ou exclure les dossiers techniques de la synchro.
- **Un seul exemplaire de chaque outil.** Deux installations d'Ollama tournaient en parallèle, l'une trop ancienne pour les modèles récents. Les erreurs qui en sortent ne disent jamais « vous avez deux Ollama ».

### 4. Donner une mémoire au projet dès le premier jour

Ce qu'on fait :
- un fichier `CLAUDE.md` à la racine : ce que fait l'app, comment elle est construite, les décisions et les pièges. Claude Code le lit à chaque session ;
- un historique des versions avec git, dès la première ligne de code.

Les garde-fous :
- À la fin de chaque étape, demander à Claude de mettre à jour le `CLAUDE.md`, dates comprises. C'est ce qui permet de reprendre à froid, sur un autre Mac ou trois mois plus tard.
- Enregistrer une version à chaque étape qui marche, pour pouvoir revenir en arrière.

Dans le traducteur : le `CLAUDE.md` a bien joué son rôle, il a permis de reprendre le projet après trois mois d'arrêt. En revanche, le projet n'a pas eu de git avant sa version publique : aucun retour arrière possible pendant quatre mois. C'est l'erreur à ne pas reproduire.

### 5. Avancer par petites fonctions visibles, la maquette d'abord

Ce qu'on fait : une fonction à la fois, décrite par ce que voit l'utilisateur. Pour un élément d'interface nouveau, demander d'abord une maquette cliquable, la valider, puis construire.

Le garde-fou : refuser les paquets de cinq changements d'un coup. Si quelque chose casse, on ne sait plus lequel.

Dans le traducteur : la pastille de dictée a d'abord été une maquette interactive (ses états, sa taille au repos). Elle a été validée avant la moindre ligne de code de l'app, et construite en une journée.

### 6. Les autorisations macOS : le vrai métier du développeur Mac

C'est le point où une app Mac diffère le plus d'un site web. Dès que l'app écoute le clavier, colle du texte, utilise le micro ou capte le son, macOS demande une autorisation. Et il la retire sans prévenir.

Ce qu'on fait : lister avec Claude les autorisations nécessaires avant de construire. Chacune est une décision produit, avec un coût pour l'utilisateur.

Les garde-fous :
- **Viser le minimum.** Le traducteur s'en tient à deux autorisations, Accessibilité et Micro. L'écoute de la touche Ctrl a été construite pour ne pas demander la « surveillance du clavier » en plus.
- **La signature.** macOS rattache une autorisation à la signature de l'app. Sans compte développeur Apple, chaque construction change de signature, et l'autorisation saute à chaque nouvelle version : elle reste affichée cochée, mais elle est refusée. La parade est une signature à identité fixe, posée par le script d'installation.
- **Le mode développement trompe.** Pendant le développement, c'est le Terminal qui porte les autorisations, pas l'app. Ce qui marche en développement peut échouer une fois installé, et l'inverse.
- **Savoir remettre à zéro.** `tccutil reset Accessibility <identifiant de l'app>` règle la plupart des « c'est coché mais ça ne marche pas ».

### 7. Tester en conditions réelles, et mesurer

Ce qu'on fait : installer l'app comme un utilisateur, s'en servir pour de vrai, avec son matériel, ses autres logiciels, sa journée.

Les garde-fous :
- **Un aperçu n'est pas un test.** L'icône de la barre de menus n'est jamais apparue dans l'app installée, alors qu'elle s'affichait en aperçu. Personne ne l'avait vérifié avant.
- **Le matériel compte.** Un casque Bluetooth met une à trois secondes à ouvrir son micro : les premiers mots étaient perdus. Garder le micro ouvert résolvait ça, mais laissait le casque en mode appel. Seul l'usage réel l'a montré.
- **Les autres logiciels aussi.** Un autre outil de dictée (Superwhisper) baissait le volume du Mac et le laissait à zéro. Le traducteur a d'abord été accusé à tort.
- **Mesurer avant de choisir.** Un second moteur de transcription semblait meilleur sur le papier. Mesuré, il était cinq fois plus lent et pas meilleur : écarté. Demander à Claude un petit script de mesure plutôt qu'un avis.
- **Exiger une preuve.** Ne pas accepter « c'est prêt » de Claude sans savoir ce qui a été vérifié : construit, lancé, essayé avec le vrai micro ? Lui demander de dire ce qu'il n'a pas pu vérifier.

### 8. Signer, installer, mettre à jour

Ce qu'on fait : un script unique qui construit l'app, la signe, la copie dans Applications et la relance. Une mise à jour revient à relancer le même script.

Les garde-fous :
- Ne jamais installer à la main une version construite autrement : on perd la signature fixe, donc les autorisations (étape 6).
- Tester l'installation sur le deuxième Mac, ou chez quelqu'un d'autre : c'est là qu'on découvre ce qui ne venait que de sa propre machine.

### 9. Des garde-fous dans le code : l'app ne doit jamais coincer l'utilisateur

Ce qu'on demande explicitement à Claude :
- **Une limite de temps sur chaque appel à l'IA.** Un modèle mal configuré générait sans fin et figeait la fenêtre. Depuis, aucun appel ne bloque l'app plus de deux minutes.
- **Remettre les choses comme on les a trouvées.** Le son baissé pendant la dictée est noté avant d'être baissé, et rendu même si l'app plante (au démarrage suivant).
- **Ne jamais garder le contenu.** Le journal de la dictée note les durées, jamais le texte dicté.
- **Laisser repartir après une erreur.** Après un « Rien entendu », la dictée restait bloquée quatre secondes. On a corrigé pour que l'appui suivant reparte tout de suite.
- **Des tests automatiques sur la logique pure** : découpage de la voix aux pauses, corrections de mots. Ils tournent en une seconde et protègent des régressions.

### 10. Rendre l'app partageable

Ce qu'on fait, quand l'outil sert à d'autres :
- **Retirer ce qui est propre à soi** : chemins vers ses propres dossiers, listes de mots personnelles, réglages en dur. Le traducteur lisait sa liste de corrections dans le dossier de travail de son auteur.
- **Lire les licences**, surtout celles des modèles d'IA. Le modèle de traduction par défaut interdit l'usage commercial ; un autre est conseillé pour un usage professionnel.
- **Écrire le mode d'emploi pour quelqu'un qui n'a pas suivi le projet** : configuration requise, installation, autorisations, dépannage, désinstallation.
- **Dire les limites** : ici, Mac à puce Apple seulement, pas de version PC, app non validée par Apple.
- **Choisir un nom et un identifiant distincts** de sa version de travail, pour que les deux puissent cohabiter.

## La liste à cocher avant de dire « c'est prêt »

1. L'app a été installée par le script, pas lancée en mode développement.
2. Elle a servi pour de vrai, avec le vrai matériel (micro, casque, écran).
3. Les autorisations tiennent après une mise à jour.
4. Une erreur (IA éteinte, micro absent, rien dit) affiche un message et laisse repartir.
5. Rien de personnel ni de secret dans le code ou les journaux.
6. Le `CLAUDE.md` dit ce qui a changé, et pourquoi.
7. Une version est enregistrée dans git.
8. On sait ce qui n'a pas été vérifié, et on l'a écrit.

## Ce que Claude Code fait bien, et ce qui reste à toi

Claude Code écrit le code, lit la documentation des outils, diagnostique une erreur à partir des journaux du Mac, écrit les scripts de mesure et de construction, et tient le `CLAUDE.md` à jour.

Ce qui reste à toi : savoir à quoi sert l'outil, trancher les choix produit (quelle autorisation, quel raccourci, quel compromis vitesse et qualité), t'en servir pour de vrai, et refuser « c'est prêt » tant que la liste ci-dessus n'est pas cochée.

## Pour aller plus loin

- **Distribuer largement** : un compte développeur Apple (99 dollars par an) permet de signer et de faire valider l'app par Apple (notarisation). Plus de commande Terminal à l'installation, et des autorisations stables sans astuce.
- **Les Mac Intel** : construire une version « universelle », qui tourne sur les deux types de puces.
- **Windows** : Tauri et les moteurs d'IA existent sur PC. Tout ce qui touche au système est à refaire : écoute du clavier, collage, volume, fenêtre flottante.
