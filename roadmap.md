# Roadmap

- [x] Compléter le démarrage et l'arrêt du proxy via l'UI
- [x] Permettre la récupération d'un fichier sur le système
- [x] Permettre à l'application d'exposer un fichier PAC sur un server http
- [x] Mettre en place l'ui pour les settings
- [x] Fiabiliser le fonctionnement du proxy en mode DIRECT (sans authentification)
- [ ] Mettre en place le fonctionnement du proxy en mode SYSTEM (sans authentification)
- [ ] Mettre en place le fonctionnement du proxy en mode MANUAL (sans authentification)
- [ ] Mettre en place le fonctionnement du proxy en mode PAC avec récupération du fichier via url "http://" / "https://" (sans authentification)
- [ ] Permettre à l'application de forcer l'utilisation d'un proxy-pac sur le système (windows only)
- [ ] Fiabiliser le fonctionnement du proxy en mode PAC (sans authentification)
- [ ] Mettre en place le ci/cd avec signature du binaire + release github
- [ ] Mettre en place cd pour la release winget
- [ ] Fiabiliser l'authentification (Basic, NTLM, Windows Session, etc.), permettre à l'utilisateur de saisir username et password dans l'UI, lorsque la méthode d'authentification du serveur le permet, ce couple username / password sera utilisé, si une erreur d'authentification ou si la méthode d'authentification ne permet pas d'authentifier l'utilisateur avec un couple user/password, l'utilisateur sera invité à saisir les informations manquantes une seule et unique nouvelle fois avant de provoquer l'échec définitif de processus.
- [ ] Ajouter des TU / TI
- [ ] Ajouter un testeur de flux
- [ ] Ajouter une liste d'exclusion en mode MANUEL
- [ ] Améliorer les logs du proxy et fournir un logfile
- [ ] Faire fonctionner les settings prévus mais non fonctionnel
