# Revue actualisée — 16 novembre 2026

Périmètre : modifications locales indexées et non indexées par rapport à `dcf8607`, leurs consommateurs, et vérification des remontées historiques ci-dessous. Aucun code applicatif n'a été modifié pendant cette revue. Les constats du 15 novembre sont conservés en historique ; leurs cases ne constituent plus l'état courant.

## Défauts des corrections actuelles

### [P1] Rétablir la compilation des deux méthodes d'arrêt

- **Emplacement :** [PAC, ligne 128](src-tauri/src/pac/server.rs#L128) et [proxy, ligne 145](src-tauri/src/proxy/server.rs#L145).
- **Problème :** le regroupement des deux `if let` laisse une accolade fermante supplémentaire dans chacune des méthodes `shutdown`. Le journal de fin d'arrêt se retrouve hors de la méthode.
- **Impact :** le backend ne compile plus. `cargo test --offline --lib` échoue avec `unexpected closing delimiter` dans le serveur PAC ; le même défaut est visible dans le proxy.
- **Correction proposée :** supprimer l'accolade devenue inutile dans chaque méthode, puis relancer la compilation et les vérifications réseau. La mention historique « compilation vérifiée » de RG-06 ne s'applique pas à cet état local.

### [P1] Limiter la validation du port amont au mode MANUAL

- **Emplacement :** [formulaire proxy, ligne 80](src/app/home/proxy-server/proxy-server.component.ts#L80).
- **Problème :** `proxyPort` vaut `0` sur une installation vierge et reçoit maintenant `Validators.min(1)`. Ce contrôle reste activé en DIRECT ; `updateProxyModeControl` retire uniquement `required`, pas les bornes.
- **Impact :** le formulaire DIRECT est invalide par défaut et `startProxyServer` refuse de démarrer. Le champ responsable est caché dans l'onglet MANUAL. SYSTEM et PAC subissent aussi cette validation inutile.
- **Correction proposée :** appliquer les validateurs du port amont uniquement en MANUAL, ou désactiver les contrôles non pertinents avec une gestion cohérente du verrouillage du formulaire.
- **Vérification :** reproduction avec les classes Angular installées : port amont `0`, erreur `min`, groupe invalide même après retrait de `required`.

### [P2] Réouvrir RG-08 : refuser les schémas et ports non pris en charge

- **Emplacement :** [transport HTTP, ligne 222](src-tauri/src/proxy/server.rs#L222).
- **Problème :** `https://` sélectionne désormais 443 mais la connexion reste un `TcpStream` avec un handshake HTTP/1 sans TLS. Aucun schéma n'est refusé. Par ailleurs, dans la dépendance locale `http 1.5.0`, `Authority::port()` utilise `Port::from_str(...).ok()` : un port non représentable retourne `None`, comme un port absent. Remplacer `port_u16()` par `port().as_u16()` ne résout donc pas cette ambiguïté.
- **Impact :** une requête HTTPS en forme absolue est envoyée en HTTP clair au port sélectionné ; un port invalide peut encore être remplacé par le port par défaut.
- **Correction proposée :** accepter uniquement le schéma HTTP dans cette branche et valider explicitement un port présent dans l'autorité, en tenant compte d'IPv6. Maintenir HTTPS via CONNECT. Ce point préexistant reste ouvert malgré la correction annoncée ; aucune reproduction réseau n'est revendiquée tant que la compilation est cassée.

### [P2] Compléter RG-07 avec une validation entière des ports

- **Emplacement :** [port proxy, ligne 67](src/app/home/proxy-server/proxy-server.component.ts#L67) et [port PAC, ligne 51](src/app/home/pac-server/pac-server.component.ts#L51).
- **Problème :** `required`, `min` et `max` acceptent un nombre comme `3128.5`. Le backend attend un `u16` et les commandes ne refusent pas explicitement le port zéro.
- **Impact :** l'UI valide une saisie qui échoue à la désérialisation Tauri ; un appel direct de commande peut toujours choisir un port éphémère sans restitution à l'UI.
- **Correction proposée :** vérifier `Number.isInteger` et les bornes dans l'UI ; refuser zéro à la frontière des commandes publiques, tout en conservant les ports éphémères pour les serveurs utilisés par les tests.
- **Vérification :** `FormControl(3128.5, [required, min(1), max(65535)])` est valide avec Angular installé.

### [P2] Conserver les tests réseau existants pendant le remaniement du cycle de vie

- **Emplacement :** suppression de `src-tauri/src/proxy/lifecycle_tests.rs` et de sa déclaration dans [server.rs](src-tauri/src/proxy/server.rs).
- **Problème :** les deux tests déjà présents de prévention des boucles et d'arrêt/redémarrage ont été supprimés, alors que le contexte d'exécution est modifié et repris par le PAC.
- **Impact :** disparition de la couverture des connexions HTTP en cours, tunnels CONNECT, alias locaux et réutilisation du port. La future tâche « ajouter des tests » ne justifie pas la perte de ces vérifications existantes.
- **Correction proposée :** restaurer ou adapter ces tests ; compléter avec le cycle de vie PAC et une connexion cliente invalide.

## État des remontées historiques

| Remontée | État actuel | Preuve ou limite |
| --- | --- | --- |
| RG-01 — paniques PAC | Correction présente, non validée en exécution | Les erreurs réseau sont journalisées ; compilation bloquée avant les essais réseau. |
| RG-04 — port amont sauvegardé | Corrigé à la lecture | La bonne propriété `proxyPort` est sauvegardée ; rechargement non rejoué. |
| RG-05 — URL PAC désactivée | Corrigé à la lecture | Sauvegarde via `getRawValue()` ; scénario complet de rechargement non rejoué. |
| RG-06 — cycle de vie PAC | Implémentation présente, validation bloquée | Annulation, suivi/attente des tâches et arrêt avant remplacement sont présents ; erreurs de syntaxe actuelles et aucun essai réseau validé. |
| RG-07 — ports | Partiel, à réouvrir | Zéro et hors plage refusés dans les champs d'écoute ; fractions acceptées, commandes sans rejet explicite de zéro, régression DIRECT. |
| RG-08 — destinations HTTP | Non corrigé | Transport sans TLS et port invalide encore assimilable à une absence. |
| RG-10 — diagnostics | Toujours ouvert, défaut actuel préexistant | Causes système exclues de la sérialisation ; `catch` frontend ignore toujours l'erreur. Cela affecte déjà les erreurs de port occupé et de fichier local absent. |
| Tests Badge / Settings | Corrigés et vérifiés | Suite Angular : 11 tests réussis sur 11. |
| Polling P3 | Toujours ouvert, défaut actuel préexistant | `setInterval` sans libération à la destruction du composant. |
| Build production | Limitation toujours reproduite | Échec de récupération de la police Google dans cet environnement ; pas une preuve d'échec dans tous les environnements. |

## Futures itérations et limites de périmètre

- **Fonctionnalités futures :** routage MANUAL/SYSTEM/PAC, téléchargement PAC HTTP/HTTPS, authentification amont et stockage sécurisé des identifiants, paramètres système et comportements de la fenêtre, CI/CD et packaging. Leur absence n'est pas une régression introduite par les corrections revues.
- **Garde-fou actuel à conserver dans le suivi :** `proxy_mode` est stocké mais n'est pas utilisé par le transport, qui reste DIRECT. Tant que les autres modes ne sont pas implémentés, il faut les refuser ou les rendre indisponibles explicitement ; leur sélection ne doit pas faire croire qu'un proxy amont est utilisé. De même, ne pas saisir/persister de vrais secrets via le champ actuellement sauvegardé en clair dans `localStorage`. Ces comportements sont préexistants, pas de nouvelles régressions de ce diff.
- **Explicitement différés dans l'historique :** DIRECT-06 (délais/limites) et DIRECT-07 (en-têtes hop-by-hop). Ils restent des limites connues et ne sont pas reclassés comme nouvelles régressions.
- **Validation restant à faire :** essais HTTP/CONNECT réels, uploads et téléchargements, arrêt pendant transfert, serveur PAC et redémarrage, installateur release, puis tests d'authentification Windows en environnement adapté. Un test de création de composant ne couvre pas ces scénarios.

## Validation du 16 novembre 2026

| Vérification | Résultat |
| --- | --- |
| `node node_modules/@angular/cli/bin/ng.js test --watch=false` | 11 fichiers / 11 tests réussis |
| Build Angular développement | Réussi |
| Build Angular production | Échec : téléchargement de la police Google |
| `cargo test --offline --lib` | Échec de compilation : accolade surnuméraire PAC ; aucun test Rust exécuté |
| Reproduction des validateurs Angular installés | Port amont par défaut invalide hors MANUAL ; port fractionnaire accepté |
| Revue du transport et de `http 1.5.0` installé | RG-08 toujours ouvert à la lecture du code |
| Tests réseau / installateur / entreprise | Non exécutés ; aucune garantie ajoutée |

---

# Historique — suivi du 15 novembre 2026

Revue globale du 15 novembre 2026 : backend Rust, commandes Tauri, interface Angular, configuration, persistance et tests. Ce document décrit l'état constaté, pas une liste de fonctionnalités garanties.

Les identifiants RG reprennent les numéros de la revue globale. P1 désigne une priorité élevée ; P2 un défaut fonctionnel à corriger ; P3 une amélioration secondaire. Les cases restent ouvertes jusqu'à correction et vérification.

## Défauts confirmés

### RG-01 — P1 — Panique du serveur PAC pouvant arrêter l'application

- [x] À corriger.
- **Emplacement :** [serveur PAC](src-tauri/src/pac/server.rs), boucle d'acceptation et traitement des connexions ; [profil release](src-tauri/Cargo.toml).
- **Constat :** les erreurs réseau sont encore transformées en `panic!`. Le profil release utilise `panic = "abort"`.
- **Impact :** une erreur sur une connexion PAC peut arrêter toute l'application, y compris le proxy DIRECT.
- **Correction proposée :** journaliser les erreurs et fermer la connexion concernée ; traiter séparément les erreurs du listener sans paniquer.
- **Vérification attendue :** requête PAC invalide et déconnexion client prématurée sans arrêt du processus ; une requête suivante doit toujours fonctionner.

### RG-05 — P2 — URL PAC désactivée effacée du stockage

- [x] À corriger.
- **Emplacement :** [formulaire proxy](src/app/home/proxy-server/proxy-server.component.ts), `syncWithLocalStorage` et `forcePacUrlWhenExposedIsTrue`.
- **Constat :** les valeurs émises par le formulaire excluent les contrôles individuellement désactivés. L'option d'utilisation du PAC exposé désactive `proxyPacUrl`, ensuite enregistré comme une chaîne vide.
- **Impact :** l'URL affichée et utilisée par le formulaire diffère de celle sauvegardée.
- **Correction proposée :** lire `getRawValue()` lors de la sauvegarde.
- **Vérification attendue :** activer l'utilisation du PAC exposé, attendre la sauvegarde puis recharger ; l'URL doit être conservée. L'effacement a été reproduit dans un DOM de test avec Tauri simulé.

### RG-06 — P2 — Cycle de vie incomplet du serveur PAC

- [x] Correction implémentée. Compilation vérifiée ; scénario réseau encore à valider.
- **Emplacement :** [serveur PAC](src-tauri/src/pac/server.rs), `shutdown` ; [commandes Tauri](src-tauri/src/commands.rs), `start_proxy_pac_server` et `stop_proxy_pac_server`.
- **Constat :** seul le listener est annulé, sans attente de terminaison ni arrêt des connexions acceptées. Un nouveau listener est créé avant l'arrêt de l'ancien.
- **Impact :** des connexions existantes peuvent continuer à servir le PAC après l'arrêt ; remplacer un serveur actif sur le même port échoue.
- **Correction proposée :** reprendre la politique d'arrêt suivie et attendue du proxy, et sérialiser les transitions du serveur PAC.
- **Vérification attendue :** fermeture des connexions actives à l'arrêt et redémarrage fiable sur le même port.

- **Réalisation :** contexte d’exécution conservé dans chaque serveur ; annulation et attente des tâches PAC ; arrêt de l’ancien listener avant remplacement sous le verrou de l’état applicatif. Les paniques réseau du serveur PAC ont également été remplacées par des logs.

### RG-07 — P2 — Port d'écoute zéro accepté sans restitution du port réel

- [x] Corrigé.
- **Emplacement :** [formulaire proxy](src/app/home/proxy-server/proxy-server.component.ts), [formulaire PAC](src/app/home/pac-server/pac-server.component.ts) et [commandes Tauri](src-tauri/src/commands.rs).
- **Constat :** les formulaires acceptent `0`. Le système choisit alors un port disponible, mais les commandes ne renvoient pas ce port à l'interface.
- **Impact :** l'utilisateur ne connaît pas le port à configurer dans son navigateur.
- **Correction proposée :** exiger un entier entre 1 et 65535 dans l'interface, ou exposer explicitement le port attribué si cette fonctionnalité est souhaitée. Le port zéro reste utile pour les tests internes.
- **Vérification attendue :** rejet de zéro, des valeurs hors plage et des nombres fractionnaires dans les deux formulaires.

### RG-08 — P2 — Validation insuffisante des destinations HTTP

- [x] Corrigé.
- **Emplacement :** [transport proxy](src-tauri/src/proxy/server.rs), branche hors CONNECT de `proxy_req`.
- **Constat :** absence de contrôle du schéma ; `port_u16().unwrap_or(80)` ne distingue pas un port absent d'un port explicitement invalide.
- **Impact :** une cible `https://` peut être traitée comme du HTTP sur TCP sans TLS ; un port invalide peut devenir silencieusement 80.
- **Correction proposée :** accepter uniquement HTTP dans cette branche et refuser les ports explicitement invalides. Conserver HTTPS via CONNECT.
- **Vérification attendue :** requêtes invalides refusées avant connexion ; HTTP sans port utilise 80 et CONNECT continue de fonctionner.

### RG-10 — P2 — Diagnostics perdus avant affichage

- [ ] À corriger.
- **Emplacement :** [erreurs des commandes](src-tauri/src/commands.rs) et [service frontend](src/app/shared/services/server.service.ts).
- **Constat :** les causes système ne sont pas sérialisées et les blocs `catch` ignorent l'erreur reçue.
- **Impact :** port occupé, fichier absent et source non supportée aboutissent à des messages génériques ne permettant pas de corriger la configuration.
- **Correction proposée :** transmettre un code et un message exploitables, sans secrets, puis les afficher.
- **Vérification attendue :** messages distincts et utiles pour ces trois échecs.

## Vérifications et maintenance encore ouvertes

- [ ] **Tests Angular :** corriger la préparation du [test Badge](src/app/shared/components/design-system/badge/badge.component.spec.ts) en fournissant `connexionStatus`, et celle du [test Settings](src/app/shared/components/modals/settings/settings.component.spec.ts) en fournissant `MatDialogRef`. Ces deux échecs ne démontrent pas un défaut des composants dans l'application réelle.
- [ ] **Build production :** résoudre ou documenter la dépendance au téléchargement des polices Google déclarées dans [index.html](src/index.html). Le build a échoué dans l'environnement de revue à cette étape ; l'hébergement local des polices permettrait un build sans ce téléchargement.
- [ ] **Polling — P3 :** libérer le `setInterval` à la destruction de [ServerStatusComponent](src/app/shared/components/common/server-status/server-status.component.ts), afin d'éviter des appels persistants après recréation du composant.
- [ ] **Validation de bout en bout :** navigation HTTP/HTTPS réelle, formulaire et upload, téléchargement volumineux, plusieurs connexions simultanées, destination indisponible, arrêt/redémarrage pendant un transfert.
- [ ] **Packaging :** vérifier un build et un installateur release. Aucun installateur ni environnement d'entreprise n'a été validé pendant cette revue.

## Sujets volontairement différés

Les numéros historiques ci-dessous concernent la première revue du mode DIRECT, pas les identifiants RG de la revue globale.

- [ ] **DIRECT-06 — Délais et limites de ressources : différé à la demande de l'utilisateur.** Aucun délai applicatif général ni plafond de concurrence n'a été ajouté. Un délai de connexion pourra être envisagé selon les essais ; une politique d'inactivité des tunnels doit préserver les connexions légitimement silencieuses.
- [ ] **DIRECT-07 — Traitement des en-têtes de connexion : correction annulée à la demande de l'utilisateur.** Le filtrage général de `Connection`, des champs qu'il désigne et des autres en-têtes hop-by-hop reste à traiter pour le HTTP classique. La prise en charge des corps, trailers et changements de protocole doit être définie avant toute nouvelle correction. Le trafic opaque à l'intérieur d'un tunnel CONNECT n'est pas concerné. La suppression de `Proxy-Authorization`, déjà corrigée séparément, reste en place.

## Fonctionnalités prévues, non implémentées

Les [paramètres](src/app/shared/components/modals/settings/settings.component.ts) de démarrage automatique, minimisation et configuration système ne sont pas opérationnels ; le bouton Save n'a pas de traitement. La [roadmap](roadmap.md) le reconnaît. Ce sont des travaux prévus, pas des régressions : signaler leur indisponibilité dans l'interface tant qu'ils ne sont pas implémentés.

## État de référence de la revue

| Vérification | Résultat constaté le 15 novembre 2026 |
| --- | --- |
| `cargo test --offline --lib` | 2 tests réussis : boucles et arrêt/redémarrage |
| Build Angular développement | Réussi |
| Tests Angular | 9 réussis, 2 échoués |
| Build Angular production | Échec de téléchargement des polices Google |
| Clippy | Terminé avec 7 avertissements, principalement de style |
| Persistance dans un DOM simulé | Mauvais port amont et effacement de l'URL PAC reproduits |

**Exclusion des points ouverts :** RG-04 (mauvais champ sauvegardé comme port amont) a été corrigé dans la modification locale présente à la création de ce document : `server_form_proxy_port` reçoit désormais `proxyPort`. Cette correction a été constatée à la lecture du code ; le scénario de rechargement n'a pas été rejoué depuis. Les corrections DIRECT 1 à 5 déjà conservées ne sont pas remises dans la liste des défauts ouverts.
