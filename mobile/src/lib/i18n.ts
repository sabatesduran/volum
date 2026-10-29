import { useEffect, useState } from "react";

export type AppLanguage = "en" | "ca" | "es";

const dictionaries: Record<Exclude<AppLanguage, "en">, Record<string, string>> = {
  ca: {
    "Library": "Biblioteca", "Favorites": "Preferits", "Upload": "Puja", "Capture": "Captura", "Collections": "Col·leccions", "Settings": "Configuració",
    "Search projects": "Cerca projectes", "All projects": "Tots els projectes", "Load more projects": "Carrega més projectes", "No projects found": "No s'ha trobat cap projecte",
    "Try another search, folder, or filter.": "Prova una altra cerca, carpeta o filtre.", "Filter & sort": "Filtra i ordena", "Folder": "Carpeta", "Saved search": "Cerca desada",
    "Format": "Format", "All": "Tots", "Tag": "Etiqueta", "All tags": "Totes les etiquetes", "Sort": "Ordre", "Recently modified": "Modificats recentment",
    "Recently added": "Afegits recentment", "Last opened": "Oberts per darrera vegada", "Name A–Z": "Nom A–Z", "Clear": "Neteja", "Show projects": "Mostra els projectes",
    "Available offline": "Disponible fora de línia", "Online only": "Només en línia", "Download": "Descarrega", "Remove": "Elimina", "Workshop mode": "Mode taller",
    "View in 3D": "Visualitza en 3D", "Notes": "Notes", "Photos": "Fotos", "Files": "Fitxers", "Source": "Font", "Print cost": "Cost d'impressió",
    "Language": "Idioma", "Connection and privacy": "Connexió i privadesa", "Unpair this device": "Desvincula aquest dispositiu", "Connected": "Connectat",
    "App permissions": "Permisos de l'app", "Manage camera and notification access in your device settings.": "Gestiona l'accés a la càmera i a les notificacions als ajustos del dispositiu.", "Open device settings": "Obre els ajustos del dispositiu", "PRIVACY & SAFETY": "PRIVADESA I SEGURETAT",
    "Recent": "Recents", "Photo": "Foto", "3D file": "Fitxer 3D", "Project": "Projecte", "Caption": "Llegenda",
    "Take photo": "Fes una foto", "Photo library": "Fototeca", "Filter projects": "Filtra projectes", "Choose a project": "Tria un projecte",
    "What does this photo show?": "Què mostra aquesta foto?", "Sending photo": "S'està enviant la foto", "Uploading": "S'està pujant",
    "Add photo to project": "Afegeix la foto al projecte", "Selected workshop photo": "Foto de taller seleccionada", "Photo added": "Foto afegida", "Saved to": "Desada a",
    "Saved to your Volum project.": "Desada al teu projecte de Volum.", "Volum Desktop validates and stores the original.": "Volum Desktop valida i desa l'original.",
    "Enable organization writes in Volum Desktop to add photos.": "Activa els canvis d'organització a Volum Desktop per afegir fotos.",
    "Sent directly through your Tailnet. No Volum cloud or relay.": "Enviat directament per la teva Tailnet. Sense núvol ni relay de Volum.",
    "Dimensions": "Dimensions", "Modified": "Modificat", "Project size": "Mida del projecte", "Edit": "Edita", "Add tags": "Afegeix etiquetes",
    "Print setup": "Configuració d'impressió", "Geometry & inspection": "Geometria i inspecció", "Workshop photos": "Fotos del taller",
    "Project details": "Detalls del projecte", "Bill of materials": "Llista de materials", "Assembly guide": "Guia de muntatge", "Related projects": "Projectes relacionats",
    "Add notes": "Afegeix notes", "Edit notes": "Edita les notes", "Save notes": "Desa les notes", "Cancel": "Cancel·la", "No notes yet.": "Encara no hi ha notes.",
    "Take a photo": "Fes una foto", "Add item": "Afegeix un element", "Add step": "Afegeix un pas", "Open source page": "Obre la pàgina d'origen",
    "Private by design": "Privat per disseny", "Safe file uploads": "Pujades de fitxers segures", "Offline storage": "Emmagatzematge fora de línia",
    "Completion alerts": "Avisos de finalització", "Clear offline projects": "Esborra els projectes fora de línia", "On": "Activats", "Off": "Desactivats",
    "Recent media cache": "Memòria cau multimèdia recent", "Clear recent media": "Esborra el contingut recent", "item": "element", "items": "elements",
    "Automatically limited to 96 MB": "Limitada automàticament a 96 MB", "No recently viewed media is cached on this device.": "No hi ha contingut multimèdia recent a la memòria cau d'aquest dispositiu.",
    "Create collection": "Crea una col·lecció", "Edit collection": "Edita la col·lecció", "Delete": "Elimina", "Save": "Desa", "Create": "Crea",
    "Your collections": "Les teves col·leccions", "Available offline projects": "Projectes disponibles fora de línia", "Keep this view offline": "Mantén aquesta vista fora de línia",
    "Project tags": "Etiquetes del projecte", "New tag": "Etiqueta nova", "Manage tag": "Gestiona l'etiqueta", "Save tag": "Desa l'etiqueta",
    "Cancel upload": "Cancel·la la pujada", "Canceling upload": "S'està cancel·lant la pujada", "Canceling…": "S'està cancel·lant…",
    "The upload is still starting or has already finished. Try again if it remains active.": "La pujada encara s'està iniciant o ja ha acabat. Torna-ho a provar si continua activa.",
    "MOBILE": "MÒBIL", "Your private 3D library": "La teva biblioteca 3D privada", "From desktop": "De l'escriptori", "to workbench.": "al banc de treball.",
    "Keep project files, photos, parts, and build steps close while you print.": "Tingues a mà els fitxers, les fotos, les peces i els passos de muntatge mentre imprimeixes.",
    "Connect to Volum Desktop": "Connecta amb Volum Desktop", "Open Settings → Companion Access on your Mac or PC.": "Obre Configuració → Accés complementari al Mac o PC.",
    "Pairing data": "Dades de vinculació", "Paste from Volum Desktop": "Enganxa des de Volum Desktop", "Connect securely": "Connecta de manera segura",
    "Use the camera instead": "Fes servir la càmera", "Scan desktop code": "Escaneja el codi de l'ordinador", "Enter pairing data manually": "Introdueix les dades manualment",
    "No account, cloud, or public relay.": "Sense compte, núvol ni relay públic.", "Camera access is needed to scan the pairing code.": "Cal accés a la càmera per escanejar el codi de vinculació.",
    "Could not load Volum": "No s'ha pogut carregar Volum", "Could not connect to Volum Desktop": "No s'ha pogut connectar amb Volum Desktop", "Try again": "Torna-ho a provar", "Connecting…": "S'està connectant…",
    "Check that Tailscale is connected on both devices, Volum Desktop is open, and Companion Access is on.": "Comprova que Tailscale estigui connectat als dos dispositius, que Volum Desktop estigui obert i que l'Accés complementari estigui activat.",
    "ORGANIZE": "ORGANITZA", "Projects, smart views, and offline packs": "Projectes, vistes intel·ligents i paquets fora de línia",
    "Manual and smart groups from Desktop": "Grups manuals i intel·ligents de l'ordinador", "Smart collection": "Col·lecció intel·ligent", "Collection": "Col·lecció", "projects": "projectes",
    "A PLACE FOR EVERY PROJECT": "UN LLOC PER A CADA PROJECTE", "No collections yet": "Encara no hi ha col·leccions", "Collections group builds, parts, and ideas without moving a file.": "Les col·leccions agrupen muntatges, peces i idees sense moure cap fitxer.",
    "Create a collection": "Crea una col·lecció", "device storage used": "d'emmagatzematge del dispositiu utilitzat", "Synced": "Sincronitzat",
    "Open a project, folder, or collection and choose": "Obre un projecte, una carpeta o una col·lecció i tria", "COLLECTION": "COL·LECCIÓ", "Name": "Nom", "Color": "Color",
    "Delete “{name}”? Projects and files will not be deleted.": "Vols eliminar «{name}»? Els projectes i els fitxers no s'eliminaran.", "Remove {name} from offline storage": "Elimina {name} de l'emmagatzematge fora de línia",
    "Edit {name}": "Edita {name}", "Close": "Tanca", "Use {color}": "Utilitza {color}",
    "project": "projecte", "Tailnet address": "Adreça de Tailnet", "Port 43821 is added automatically.": "El port 43821 s'afegeix automàticament.",
    "Volum follows this language for the mobile interface.": "Volum utilitza aquest idioma a la interfície mòbil.",
    "Your token is encrypted on this device. Library requests travel through your Tailnet without a Volum cloud or relay.": "El testimoni està xifrat en aquest dispositiu. Les peticions de la biblioteca viatgen per la teva Tailnet sense núvol ni relay de Volum.",
    "Selected 3D files are validated and imported into your managed library. Destructive actions remain on Desktop.": "Els fitxers 3D seleccionats es validen i s'importen a la biblioteca gestionada. Les accions destructives continuen a l'ordinador.",
    "Get a local alert when a file, photo, or offline pack finishes while Volum is open.": "Rep un avís local quan acaba un fitxer, una foto o un paquet fora de línia mentre Volum és obert.",
    "Checking permission…": "S'està comprovant el permís…", "No project snapshots are stored on this device.": "No hi ha cap instantània de projecte desada en aquest dispositiu.",
    "That address belongs to a different Volum Desktop.": "Aquesta adreça pertany a un altre Volum Desktop.", "Notification permission was not granted.": "No s'ha concedit el permís de notificacions.",
    "Remove {count} offline {projects} from this device?": "Vols eliminar {count} {projects} fora de línia d'aquest dispositiu?",
    "Remove {size} of recently viewed media from this device? Offline projects will remain available.": "Vols eliminar {size} de contingut recent d'aquest dispositiu? Els projectes fora de línia continuaran disponibles.",
    "file": "fitxer", "files": "fitxers", "Your private collection": "La teva col·lecció privada", "Scan project code": "Escaneja el codi del projecte", "Open project scanner": "Obre l'escàner de projectes",
    "QR scanner": "Escàner QR", "Cancel scan": "Cancel·la l'escaneig", "Scan the pairing code": "Escaneja el codi de vinculació", "Point the camera at the code shown in Volum Desktop. You can cancel at any time.": "Apunta la càmera al codi que mostra Volum Desktop. Pots cancel·lar en qualsevol moment.",
    "PROJECT QR": "QR DE PROJECTE", "Open a shared project": "Obre un projecte compartit", "Scan a project QR shown by Volum Desktop or another device paired with this same Desktop.": "Escaneja un QR de projecte que mostri Volum Desktop o un altre dispositiu vinculat amb aquest mateix ordinador.",
    "This does not pair the app with Volum Desktop. Pairing codes are scanned from the connection screen.": "Això no vincula l'app amb Volum Desktop. Els codis de vinculació s'escanegen des de la pantalla de connexió.", "Scan project QR": "Escaneja el QR del projecte", "Scan a project QR": "Escaneja un QR de projecte", "Point the camera at a project code shown in Volum Desktop or another paired Volum device.": "Apunta la càmera a un codi de projecte mostrat a Volum Desktop o en un altre dispositiu Volum vinculat.",
    "Showing saved projects": "Es mostren els projectes desats", "Volum Desktop is unavailable. Search and basic filters apply to this device’s offline snapshots.": "Volum Desktop no està disponible. La cerca i els filtres bàsics s'apliquen a les instantànies fora de línia d'aquest dispositiu.",
    "Search": "Cerca", "Filter and sort": "Filtra i ordena", "Download every project in this collection or folder.": "Descarrega tots els projectes d'aquesta col·lecció o carpeta.",
    "{completed} of {total} projects": "{completed} de {total} projectes", "BROWSE": "EXPLORA", "Close filters": "Tanca els filtres", "All folders": "Totes les carpetes", "None": "Cap",
    "Desktop file": "Fitxer de l'ordinador", "Any": "Qualsevol", "Available": "Disponible", "Missing": "No disponible",
    "{count} {projects} could not be saved. {details}": "No s'han pogut desar {count} {projects}. {details}", "{count} more": "{count} més",
    "Offline download finished with errors": "La descàrrega fora de línia ha acabat amb errors", "Offline download complete": "Descàrrega fora de línia completada",
    "{saved} of {total} projects were saved.": "S'han desat {saved} de {total} projectes.", "{count} projects are ready offline.": "Hi ha {count} projectes disponibles fora de línia.",
    "Camera access is needed to scan a project code.": "Cal accés a la càmera per escanejar el codi d'un projecte.", "This project belongs to a different Volum Desktop.": "Aquest projecte pertany a un altre Volum Desktop.", "This project is no longer available.": "Aquest projecte ja no està disponible.",
    "WORKSHOP CAPTURE": "CAPTURA DE TALLER", "Add a project photo or send a new print file": "Afegeix una foto al projecte o envia un fitxer d'impressió nou", "Capture type": "Tipus de captura",
    "Choose a 3D file": "Tria un fitxer 3D", "3D files": "Fitxers 3D", "Ready for Volum Desktop": "A punt per a Volum Desktop", "STL, 3MF, OBJ, STEP, and STP up to 500 MB.": "STL, 3MF, OBJ, STEP i STP de fins a 500 MB.",
    "Choose another file": "Tria un altre fitxer", "Browse Files": "Explora els fitxers", "New project name": "Nom del projecte nou", "Uploading to Desktop": "S'està pujant a l'ordinador",
    "Keep Volum open until the file is imported.": "Mantén Volum obert fins que s'importi el fitxer.", "Upload to Volum": "Puja a Volum", "Update Volum Desktop to enable secure file uploads.": "Actualitza Volum Desktop per activar les pujades segures.",
    "3D file imported": "Fitxer 3D importat", "{filename} is now in Volum.": "{filename} ja és a Volum.",
    "3MF profile": "Perfil 3MF", "3MF project": "Projecte 3MF", "ASSEMBLY GUIDE": "GUIA DE MUNTATGE", "Add a 3D file": "Afegeix un fitxer 3D",
    "Add build notes, settings, or reminders…": "Afegeix notes de muntatge, configuracions o recordatoris…", "Add estimate": "Afegeix una estimació", "Add instructions from the project detail view first.": "Afegeix primer les instruccions des dels detalls del projecte.",
    "Add photo": "Afegeix una foto", "All plates": "Totes les plaques", "BILL OF MATERIALS": "LLISTA DE MATERIALS", "Back to library": "Torna a la biblioteca", "Back to project": "Torna al projecte",
    "Capture the build, a finished print, or an assembly detail.": "Captura el muntatge, una impressió acabada o un detall de l'assemblatge.", "Cover": "Portada", "Create a collection from the Collections tab, then add this project here.": "Crea una col·lecció des de la pestanya Col·leccions i afegeix-hi aquest projecte.",
    "Create the first tag below.": "Crea la primera etiqueta a continuació.", "Custom field": "Camp personalitzat", "Delete photo": "Elimina la foto", "Edit estimate": "Edita l'estimació", "Edit item": "Edita l'element", "Edit photo": "Edita la foto", "Edit step": "Edita el pas",
    "Favorite project": "Projecte preferit", "Included": "Inclòs", "Instructions": "Instruccions", "Layer": "Capa", "Leave workshop mode": "Surt del mode taller", "Linked photos": "Fotos vinculades", "Linked project file": "Fitxer del projecte vinculat", "Loading project": "S'està carregant el projecte",
    "Manual": "Manual", "Material": "Material", "Material estimate": "Estimació de material", "Move photo down": "Mou la foto avall", "Move photo up": "Mou la foto amunt", "Next": "Següent", "No": "No",
    "No assembly steps": "No hi ha passos de muntatge", "No assembly steps have been added yet.": "Encara no s'ha afegit cap pas de muntatge.", "No linked file": "Cap fitxer vinculat", "No parts have been added yet.": "Encara no s'ha afegit cap peça.",
    "No priced material": "Cap material amb preu", "No saved material estimate yet.": "Encara no hi ha cap estimació de material desada.", "Not set": "Sense definir", "Nozzle": "Broquet", "Opening 3D viewer…": "S'està obrint el visor 3D…",
    "Ordered build instructions linked to parts and photos.": "Instruccions de muntatge ordenades i vinculades a peces i fotos.", "PARTS FOR THIS STEP": "PECES PER A AQUEST PAS", "PRINT COST": "COST D'IMPRESSIÓ", "PROJECT": "PROJECTE", "PROJECT HANDOFF": "TRASPÀS DEL PROJECTE",
    "Part type": "Tipus de peça", "Parts used": "Peces utilitzades", "Photo details": "Detalls de la foto", "Plastic per piece (g)": "Plàstic per peça (g)", "Plate": "Placa", "Plate-specific previews need a connection to Volum Desktop. The saved full model remains available.": "Les previsualitzacions per placa necessiten connexió amb Volum Desktop. El model complet desat continua disponible.",
    "Previous": "Anterior", "Printed": "Impresa", "Printed files and purchased parts for this build.": "Fitxers impresos i peces comprades per a aquest muntatge.", "Printer": "Impressora", "Profile": "Perfil", "Project details copied to the clipboard.": "Els detalls del projecte s'han copiat al porta-retalls.",
    "Project file": "Fitxer del projecte", "Project summary": "Resum del projecte", "Purchase link": "Enllaç de compra", "Purchased": "Comprada", "Quantity": "Quantitat", "Rename project": "Canvia el nom del projecte", "Renaming…": "S'està canviant el nom…", "Reset 3D view": "Restableix la vista 3D",
    "Save details, photos, and the 3D view to this device.": "Desa els detalls, les fotos i la vista 3D en aquest dispositiu.", "Save estimate": "Desa l'estimació", "Save item": "Desa l'element", "Save name": "Desa el nom", "Save photo": "Desa la foto", "Save step": "Desa el pas", "Save tags": "Desa les etiquetes", "Saving…": "S'està desant…",
    "Scan from Library on another device paired with": "Escaneja'l des de la Biblioteca d'un altre dispositiu vinculat amb", "Scan to open": "Escaneja per obrir", "Share project": "Comparteix el projecte", "Show project QR code": "Mostra el codi QR del projecte", "Shown in the library and detail header.": "Es mostra a la biblioteca i a la capçalera dels detalls.",
    "Source page": "Pàgina d'origen", "Step title": "Títol del pas", "TAG": "ETIQUETA", "Tap to include": "Toca per incloure", "The code contains project IDs only—never your token or file paths.": "El codi només conté identificadors de projecte, mai el testimoni ni rutes de fitxers.",
    "Unit": "Unitat", "Update available": "Actualització disponible", "Use as project cover": "Utilitza-la com a portada del projecte", "View": "Visualitza", "WORKSHOP MODE": "MODE TALLER", "WORKSHOP PHOTO": "FOTO DE TALLER", "Weight source": "Origen del pes", "Whole project": "Tot el projecte", "Yes": "Sí",
    "g each": "g cadascuna", "objects": "objectes", "parts": "peces", "photos": "fotos", "this assembly step": "aquest pas de muntatge", "triangles": "triangles",
    "Delete {name}": "Elimina {name}", "Delete “{name}” from the bill of materials?": "Vols eliminar «{name}» de la llista de materials?", "Delete “{name}”?": "Vols eliminar «{name}»?", "Delete “{name}”? It will be removed from {count} projects.": "Vols eliminar «{name}»? S'eliminarà de {count} projectes.",
    "Drag to rotate · Pinch to zoom": "Arrossega per girar · Pessiga per ampliar", "Manage {name}": "Gestiona {name}", "Move {name} down": "Mou {name} avall", "Move {name} up": "Mou {name} amunt", "Plate {number}": "Placa {number}", "Preparing 3D model…": "S'està preparant el model 3D…",
    "QR code for {name}": "Codi QR de {name}", "Save {name}": "Desa {name}", "Saved to {name}.": "Desada a {name}.", "Step {number}": "Pas {number}", "This device was unpaired, but its offline cache could not be fully removed: {error}": "El dispositiu s'ha desvinculat, però no s'ha pogut eliminar del tot la memòria cau fora de línia: {error}",
    "Unpair this device and remove its cached credentials?": "Vols desvincular aquest dispositiu i eliminar-ne les credencials desades?", "Used by {count} {projects}.": "Utilitzada per {count} {projects}.", "View {name} in 3D": "Visualitza {name} en 3D", "WORKSHOP MODE · {current} OF {total}": "MODE TALLER · {current} DE {total}",
    "Enter valid device details.": "Introdueix dades de dispositiu vàlides.", "Create a new pairing code on Volum Desktop.": "Crea un codi de vinculació nou a Volum Desktop.", "Volum could not save this paired device.": "Volum no ha pogut desar aquest dispositiu vinculat.",
    "Pair this device with Volum Desktop.": "Vincula aquest dispositiu amb Volum Desktop.", "This device is not paired or has been revoked.": "Aquest dispositiu no està vinculat o se n'ha revocat l'accés.", "The pairing code is not valid.": "El codi de vinculació no és vàlid.",
    "Enable mobile organization changes in Volum Desktop settings first.": "Activa primer els canvis d'organització mòbils a la configuració de Volum Desktop.", "This project changed on another device. Refresh and try again.": "Aquest projecte ha canviat en un altre dispositiu. Actualitza'l i torna-ho a provar.",
    "The requested Volum item is unavailable.": "L'element de Volum sol·licitat no està disponible.", "The request is invalid.": "La petició no és vàlida.", "The requested change is invalid.": "El canvi sol·licitat no és vàlid.", "The selected upload is invalid.": "La pujada seleccionada no és vàlida.", "The upload size is required.": "Cal indicar la mida de la pujada.",
    "Project photos must be non-empty and at most 25 MB.": "Les fotos de projecte no poden estar buides i han de tenir com a màxim 25 MB.", "Another Companion upload is being imported. Try again shortly.": "S'està important una altra pujada del Companion. Torna-ho a provar d'aquí a poc.",
    "The model viewer is temporarily unavailable.": "El visor de models no està disponible temporalment.", "This model is too detailed for the mobile viewer.": "Aquest model té massa detall per al visor mòbil.", "3D files can be at most 500 MB.": "Els fitxers 3D poden tenir com a màxim 500 MB.",
    "Volum Desktop could not import the uploaded file.": "Volum Desktop no ha pogut importar el fitxer pujat.", "Configure a managed library on Volum Desktop before uploading files.": "Configura una biblioteca gestionada a Volum Desktop abans de pujar fitxers.",
    "Choose a managed project that is not in Trash.": "Tria un projecte gestionat que no sigui a la Paperera.", "Volum Desktop could not finish saving this upload.": "Volum Desktop no ha pogut acabar de desar aquesta pujada.", "Volum could not complete this request.": "Volum no ha pogut completar aquesta petició.",
    "Volum Desktop returned an unreadable response": "Volum Desktop ha retornat una resposta il·legible", "Could not reach Volum Desktop through Tailscale.": "No s'ha pogut contactar amb Volum Desktop mitjançant Tailscale.", "Volum Desktop rejected this upload ({status})": "Volum Desktop ha rebutjat aquesta pujada ({status})", "Volum Desktop returned {status}": "Volum Desktop ha retornat {status}",
    "This Volum Desktop identity changed during pairing.": "La identitat d'aquest Volum Desktop ha canviat durant la vinculació.", "Volum Desktop returned an invalid media response.": "Volum Desktop ha retornat una resposta multimèdia no vàlida.", "Volum Desktop returned an unreadable media response.": "Volum Desktop ha retornat una resposta multimèdia il·legible.",
    "Enter your Volum Desktop Tailnet address.": "Introdueix l'adreça Tailnet de Volum Desktop.", "That Tailnet address is not valid.": "Aquesta adreça Tailnet no és vàlida.", "Enter only the Volum Desktop Tailnet address.": "Introdueix només l'adreça Tailnet de Volum Desktop.",
    "That is not a Volum pairing code.": "Això no és un codi de vinculació de Volum.", "That is not a compatible Volum pairing code.": "Això no és un codi de vinculació de Volum compatible.", "This pairing code has expired. Create a new one on Volum Desktop.": "Aquest codi de vinculació ha caducat. Crea'n un de nou a Volum Desktop.",
    "Choose a JPEG, PNG, or WebP photo.": "Tria una foto JPEG, PNG o WebP.", "The offline snapshot identity is invalid.": "La identitat de la instantània fora de línia no és vàlida.", "This project needs more than the 250 MB offline limit.": "Aquest projecte necessita més del límit fora de línia de 250 MB.",
    "This device does not have enough storage for the offline project.": "Aquest dispositiu no té prou espai per al projecte fora de línia.", "This device is low on storage. Free at least 50 MB before saving projects offline.": "Aquest dispositiu té poc espai. Allibera almenys 50 MB abans de desar projectes fora de línia.",
    "The project identity is invalid.": "La identitat del projecte no és vàlida.", "This is not a Volum project code.": "Això no és un codi de projecte de Volum.", "This project code is invalid.": "Aquest codi de projecte no és vàlid.", "This source link is invalid.": "Aquest enllaç d'origen no és vàlid.", "Volum can only open ordinary HTTP or HTTPS source links.": "Volum només pot obrir enllaços HTTP o HTTPS ordinaris.",
    "The viewer received an incomplete model.": "El visor ha rebut un model incomplet.", "The viewer received an unsupported model.": "El visor ha rebut un model no compatible.", "The viewer received invalid model geometry.": "El visor ha rebut una geometria de model no vàlida.", "The model contains no displayable geometry.": "El model no conté cap geometria visible.", "The model has invalid geometry bounds.": "Els límits geomètrics del model no són vàlids.", "This model could not be displayed.": "No s'ha pogut mostrar aquest model.",
    "Volum Desktop did not respond. Check that it is open and Companion Access is on.": "Volum Desktop no respon. Comprova que estigui obert i que l'Accés complementari estigui activat.", "The connection to Volum Desktop failed.": "La connexió amb Volum Desktop ha fallat.", "This device is no longer paired with Volum Desktop": "Aquest dispositiu ja no està vinculat amb Volum Desktop",
    "Pair this device with Volum Desktop before uploading": "Vincula aquest dispositiu amb Volum Desktop abans de pujar fitxers", "Choose an existing project or enter a name for a new project": "Tria un projecte existent o introdueix un nom per a un projecte nou", "The selected file is no longer available": "El fitxer seleccionat ja no està disponible",
    "Pair this device and choose a project before uploading": "Vincula aquest dispositiu i tria un projecte abans de pujar fitxers", "Refresh the project and enter a shorter photo caption": "Actualitza el projecte i introdueix una llegenda de foto més curta", "The selected photo is no longer available": "La foto seleccionada ja no està disponible",
    "Choose a non-empty photo": "Tria una foto que no estigui buida", "Project photos can be at most 25 MB": "Les fotos de projecte poden tenir com a màxim 25 MB", "The upload has already finished": "La pujada ja ha acabat", "That upload is already running": "Aquesta pujada ja està en curs",
    "Choose a non-empty 3D file": "Tria un fitxer 3D que no estigui buit", "The selected file could not be inspected": "No s'ha pogut inspeccionar el fitxer seleccionat", "Volum Desktop returned an unreadable upload response": "Volum Desktop ha retornat una resposta de pujada il·legible",
    "Choose an STL, 3MF, OBJ, STEP, or STP file": "Tria un fitxer STL, 3MF, OBJ, STEP o STP", "Choose a JPEG, PNG, or WebP photo": "Tria una foto JPEG, PNG o WebP", "The Tailnet address is invalid": "L'adreça Tailnet no és vàlida", "Volum Companion only connects to Tailscale addresses": "Volum Companion només es connecta a adreces de Tailscale",
    "The file size is required before uploading.": "Cal indicar la mida del fitxer abans de pujar-lo.", "Choose a non-empty 3D file.": "Tria un fitxer 3D que no estigui buit.", "Volum Desktop could not prepare this upload.": "Volum Desktop no ha pogut preparar aquesta pujada.", "The selected project is invalid.": "El projecte seleccionat no és vàlid.", "The selected project is unavailable.": "El projecte seleccionat no està disponible.",
    "Enter a valid project name.": "Introdueix un nom de projecte vàlid.", "Choose either an existing project or a name for a new project.": "Tria un projecte existent o un nom per a un projecte nou.", "Volum Desktop could not save this upload.": "Volum Desktop no ha pogut desar aquesta pujada.", "The upload was interrupted.": "La pujada s'ha interromput.", "The uploaded file is too large.": "El fitxer pujat és massa gran.",
    "The uploaded data did not match the declared file size.": "Les dades pujades no coincideixen amb la mida de fitxer declarada.", "The upload ended before the complete file arrived.": "La pujada ha acabat abans que arribés el fitxer complet.", "The filename is not valid.": "El nom del fitxer no és vàlid.", "Choose an STL, 3MF, OBJ, STEP, or STP file.": "Tria un fitxer STL, 3MF, OBJ, STEP o STP.",
    "The photo filename is not valid.": "El nom del fitxer de foto no és vàlid.", "The photo size is required before uploading.": "Cal indicar la mida de la foto abans de pujar-la.", "Photo captions may contain up to 500 characters.": "Les llegendes de foto poden tenir fins a 500 caràcters.",
    "ready": "a punt", "pending": "pendent", "failed": "amb errors", "printable": "imprimible", "source": "origen", "reference": "referència", "version": "versió", "duplicate": "duplicat", "geometry": "geometria", "manual": "manual", "3mf": "perfil 3MF", "web": "pàgina d'origen",
    "3MF has no model document": "El 3MF no conté cap document de model", "Archive contains no supported model files": "L'arxiu no conté cap fitxer de model compatible",
    "Remove from favorites": "Elimina dels preferits", "Add to favorites": "Afegeix als preferits", "Batch estimate": "Estimació del lot", "Estimated cost": "Cost estimat", "Protocol": "Protocol", "Refreshing": "S'està actualitzant", "Pull to refresh": "Arrossega per actualitzar",
  },
  es: {
    "Library": "Biblioteca", "Favorites": "Favoritos", "Upload": "Subir", "Capture": "Captura", "Collections": "Colecciones", "Settings": "Ajustes",
    "Search projects": "Buscar proyectos", "All projects": "Todos los proyectos", "Load more projects": "Cargar más proyectos", "No projects found": "No se encontraron proyectos",
    "Try another search, folder, or filter.": "Prueba otra búsqueda, carpeta o filtro.", "Filter & sort": "Filtrar y ordenar", "Folder": "Carpeta", "Saved search": "Búsqueda guardada",
    "Format": "Formato", "All": "Todos", "Tag": "Etiqueta", "All tags": "Todas las etiquetas", "Sort": "Orden", "Recently modified": "Modificados recientemente",
    "Recently added": "Añadidos recientemente", "Last opened": "Abiertos por última vez", "Name A–Z": "Nombre A–Z", "Clear": "Limpiar", "Show projects": "Mostrar proyectos",
    "Available offline": "Disponible sin conexión", "Online only": "Solo en línea", "Download": "Descargar", "Remove": "Eliminar", "Workshop mode": "Modo taller",
    "View in 3D": "Ver en 3D", "Notes": "Notas", "Photos": "Fotos", "Files": "Archivos", "Source": "Fuente", "Print cost": "Coste de impresión",
    "Language": "Idioma", "Connection and privacy": "Conexión y privacidad", "Unpair this device": "Desvincular este dispositivo", "Connected": "Conectado",
    "App permissions": "Permisos de la app", "Manage camera and notification access in your device settings.": "Gestiona el acceso a la cámara y a las notificaciones en los ajustes del dispositivo.", "Open device settings": "Abrir ajustes del dispositivo", "PRIVACY & SAFETY": "PRIVACIDAD Y SEGURIDAD",
    "Recent": "Recientes", "Photo": "Foto", "3D file": "Archivo 3D", "Project": "Proyecto", "Caption": "Pie de foto",
    "Take photo": "Hacer foto", "Photo library": "Fototeca", "Filter projects": "Filtrar proyectos", "Choose a project": "Elegir un proyecto",
    "What does this photo show?": "¿Qué muestra esta foto?", "Sending photo": "Enviando foto", "Uploading": "Subiendo",
    "Add photo to project": "Añadir foto al proyecto", "Selected workshop photo": "Foto de taller seleccionada", "Photo added": "Foto añadida", "Saved to": "Guardada en",
    "Saved to your Volum project.": "Guardada en tu proyecto de Volum.", "Volum Desktop validates and stores the original.": "Volum Desktop valida y guarda el original.",
    "Enable organization writes in Volum Desktop to add photos.": "Activa los cambios de organización en Volum Desktop para añadir fotos.",
    "Sent directly through your Tailnet. No Volum cloud or relay.": "Enviado directamente por tu Tailnet. Sin nube ni relay de Volum.",
    "Dimensions": "Dimensiones", "Modified": "Modificado", "Project size": "Tamaño del proyecto", "Edit": "Editar", "Add tags": "Añadir etiquetas",
    "Print setup": "Configuración de impresión", "Geometry & inspection": "Geometría e inspección", "Workshop photos": "Fotos del taller",
    "Project details": "Detalles del proyecto", "Bill of materials": "Lista de materiales", "Assembly guide": "Guía de montaje", "Related projects": "Proyectos relacionados",
    "Add notes": "Añadir notas", "Edit notes": "Editar notas", "Save notes": "Guardar notas", "Cancel": "Cancelar", "No notes yet.": "Todavía no hay notas.",
    "Take a photo": "Hacer una foto", "Add item": "Añadir elemento", "Add step": "Añadir paso", "Open source page": "Abrir página de origen",
    "Private by design": "Privado por diseño", "Safe file uploads": "Subidas de archivos seguras", "Offline storage": "Almacenamiento sin conexión",
    "Completion alerts": "Avisos de finalización", "Clear offline projects": "Borrar proyectos sin conexión", "On": "Activados", "Off": "Desactivados",
    "Recent media cache": "Caché multimedia reciente", "Clear recent media": "Borrar contenido reciente", "item": "elemento", "items": "elementos",
    "Automatically limited to 96 MB": "Limitada automáticamente a 96 MB", "No recently viewed media is cached on this device.": "No hay contenido multimedia reciente en la caché de este dispositivo.",
    "Create collection": "Crear colección", "Edit collection": "Editar colección", "Delete": "Eliminar", "Save": "Guardar", "Create": "Crear",
    "Your collections": "Tus colecciones", "Available offline projects": "Proyectos disponibles sin conexión", "Keep this view offline": "Mantener esta vista sin conexión",
    "Project tags": "Etiquetas del proyecto", "New tag": "Etiqueta nueva", "Manage tag": "Gestionar etiqueta", "Save tag": "Guardar etiqueta",
    "Cancel upload": "Cancelar subida", "Canceling upload": "Cancelando subida", "Canceling…": "Cancelando…",
    "The upload is still starting or has already finished. Try again if it remains active.": "La subida todavía se está iniciando o ya ha terminado. Vuelve a intentarlo si sigue activa.",
    "MOBILE": "MÓVIL", "Your private 3D library": "Tu biblioteca 3D privada", "From desktop": "Del escritorio", "to workbench.": "al banco de trabajo.",
    "Keep project files, photos, parts, and build steps close while you print.": "Ten a mano los archivos, fotos, piezas y pasos de montaje mientras imprimes.",
    "Connect to Volum Desktop": "Conectar con Volum Desktop", "Open Settings → Companion Access on your Mac or PC.": "Abre Ajustes → Acceso complementario en tu Mac o PC.",
    "Pairing data": "Datos de vinculación", "Paste from Volum Desktop": "Pegar desde Volum Desktop", "Connect securely": "Conectar de forma segura",
    "Use the camera instead": "Usar la cámara", "Scan desktop code": "Escanear código del ordenador", "Enter pairing data manually": "Introducir los datos manualmente",
    "No account, cloud, or public relay.": "Sin cuenta, nube ni relay público.", "Camera access is needed to scan the pairing code.": "Se necesita acceso a la cámara para escanear el código de vinculación.",
    "Could not load Volum": "No se ha podido cargar Volum", "Could not connect to Volum Desktop": "No se ha podido conectar con Volum Desktop", "Try again": "Reintentar", "Connecting…": "Conectando…",
    "Check that Tailscale is connected on both devices, Volum Desktop is open, and Companion Access is on.": "Comprueba que Tailscale esté conectado en ambos dispositivos, que Volum Desktop esté abierto y que el Acceso complementario esté activado.",
    "ORGANIZE": "ORGANIZAR", "Projects, smart views, and offline packs": "Proyectos, vistas inteligentes y paquetes sin conexión",
    "Manual and smart groups from Desktop": "Grupos manuales e inteligentes del ordenador", "Smart collection": "Colección inteligente", "Collection": "Colección", "projects": "proyectos",
    "A PLACE FOR EVERY PROJECT": "UN LUGAR PARA CADA PROYECTO", "No collections yet": "Todavía no hay colecciones", "Collections group builds, parts, and ideas without moving a file.": "Las colecciones agrupan montajes, piezas e ideas sin mover ningún archivo.",
    "Create a collection": "Crear una colección", "device storage used": "de almacenamiento del dispositivo usado", "Synced": "Sincronizado",
    "Open a project, folder, or collection and choose": "Abre un proyecto, una carpeta o una colección y elige", "COLLECTION": "COLECCIÓN", "Name": "Nombre", "Color": "Color",
    "Delete “{name}”? Projects and files will not be deleted.": "¿Eliminar «{name}»? Los proyectos y archivos no se eliminarán.", "Remove {name} from offline storage": "Eliminar {name} del almacenamiento sin conexión",
    "Edit {name}": "Editar {name}", "Close": "Cerrar", "Use {color}": "Usar {color}",
    "project": "proyecto", "Tailnet address": "Dirección de Tailnet", "Port 43821 is added automatically.": "El puerto 43821 se añade automáticamente.",
    "Volum follows this language for the mobile interface.": "Volum utiliza este idioma en la interfaz móvil.",
    "Your token is encrypted on this device. Library requests travel through your Tailnet without a Volum cloud or relay.": "El token está cifrado en este dispositivo. Las solicitudes de la biblioteca viajan por tu Tailnet sin nube ni relay de Volum.",
    "Selected 3D files are validated and imported into your managed library. Destructive actions remain on Desktop.": "Los archivos 3D seleccionados se validan e importan en la biblioteca gestionada. Las acciones destructivas permanecen en el ordenador.",
    "Get a local alert when a file, photo, or offline pack finishes while Volum is open.": "Recibe un aviso local cuando termina un archivo, una foto o un paquete sin conexión mientras Volum está abierto.",
    "Checking permission…": "Comprobando permiso…", "No project snapshots are stored on this device.": "No hay instantáneas de proyectos guardadas en este dispositivo.",
    "That address belongs to a different Volum Desktop.": "Esa dirección pertenece a otro Volum Desktop.", "Notification permission was not granted.": "No se ha concedido el permiso de notificaciones.",
    "Remove {count} offline {projects} from this device?": "¿Eliminar {count} {projects} sin conexión de este dispositivo?",
    "Remove {size} of recently viewed media from this device? Offline projects will remain available.": "¿Eliminar {size} de contenido reciente de este dispositivo? Los proyectos sin conexión seguirán disponibles.",
    "file": "archivo", "files": "archivos", "Your private collection": "Tu colección privada", "Scan project code": "Escanear código del proyecto", "Open project scanner": "Abrir escáner de proyectos",
    "QR scanner": "Escáner QR", "Cancel scan": "Cancelar escaneo", "Scan the pairing code": "Escanear el código de vinculación", "Point the camera at the code shown in Volum Desktop. You can cancel at any time.": "Apunta la cámara al código que muestra Volum Desktop. Puedes cancelar en cualquier momento.",
    "PROJECT QR": "QR DE PROYECTO", "Open a shared project": "Abrir un proyecto compartido", "Scan a project QR shown by Volum Desktop or another device paired with this same Desktop.": "Escanea un QR de proyecto que muestre Volum Desktop u otro dispositivo vinculado con este mismo ordenador.",
    "This does not pair the app with Volum Desktop. Pairing codes are scanned from the connection screen.": "Esto no vincula la app con Volum Desktop. Los códigos de vinculación se escanean desde la pantalla de conexión.", "Scan project QR": "Escanear QR del proyecto", "Scan a project QR": "Escanear un QR de proyecto", "Point the camera at a project code shown in Volum Desktop or another paired Volum device.": "Apunta la cámara a un código de proyecto mostrado en Volum Desktop o en otro dispositivo Volum vinculado.",
    "Showing saved projects": "Mostrando proyectos guardados", "Volum Desktop is unavailable. Search and basic filters apply to this device’s offline snapshots.": "Volum Desktop no está disponible. La búsqueda y los filtros básicos se aplican a las instantáneas sin conexión de este dispositivo.",
    "Search": "Buscar", "Filter and sort": "Filtrar y ordenar", "Download every project in this collection or folder.": "Descarga todos los proyectos de esta colección o carpeta.",
    "{completed} of {total} projects": "{completed} de {total} proyectos", "BROWSE": "EXPLORAR", "Close filters": "Cerrar filtros", "All folders": "Todas las carpetas", "None": "Ninguna",
    "Desktop file": "Archivo del ordenador", "Any": "Cualquiera", "Available": "Disponible", "Missing": "No disponible",
    "{count} {projects} could not be saved. {details}": "No se pudieron guardar {count} {projects}. {details}", "{count} more": "{count} más",
    "Offline download finished with errors": "La descarga sin conexión terminó con errores", "Offline download complete": "Descarga sin conexión completada",
    "{saved} of {total} projects were saved.": "Se guardaron {saved} de {total} proyectos.", "{count} projects are ready offline.": "Hay {count} proyectos disponibles sin conexión.",
    "Camera access is needed to scan a project code.": "Se necesita acceso a la cámara para escanear el código de un proyecto.", "This project belongs to a different Volum Desktop.": "Este proyecto pertenece a otro Volum Desktop.", "This project is no longer available.": "Este proyecto ya no está disponible.",
    "WORKSHOP CAPTURE": "CAPTURA DE TALLER", "Add a project photo or send a new print file": "Añade una foto al proyecto o envía un archivo de impresión nuevo", "Capture type": "Tipo de captura",
    "Choose a 3D file": "Elegir un archivo 3D", "3D files": "Archivos 3D", "Ready for Volum Desktop": "Listo para Volum Desktop", "STL, 3MF, OBJ, STEP, and STP up to 500 MB.": "STL, 3MF, OBJ, STEP y STP de hasta 500 MB.",
    "Choose another file": "Elegir otro archivo", "Browse Files": "Explorar archivos", "New project name": "Nombre del proyecto nuevo", "Uploading to Desktop": "Subiendo al ordenador",
    "Keep Volum open until the file is imported.": "Mantén Volum abierto hasta que se importe el archivo.", "Upload to Volum": "Subir a Volum", "Update Volum Desktop to enable secure file uploads.": "Actualiza Volum Desktop para activar las subidas seguras.",
    "3D file imported": "Archivo 3D importado", "{filename} is now in Volum.": "{filename} ya está en Volum.",
    "3MF profile": "Perfil 3MF", "3MF project": "Proyecto 3MF", "ASSEMBLY GUIDE": "GUÍA DE MONTAJE", "Add a 3D file": "Añadir un archivo 3D",
    "Add build notes, settings, or reminders…": "Añade notas de montaje, ajustes o recordatorios…", "Add estimate": "Añadir estimación", "Add instructions from the project detail view first.": "Añade primero las instrucciones desde los detalles del proyecto.",
    "Add photo": "Añadir foto", "All plates": "Todas las placas", "BILL OF MATERIALS": "LISTA DE MATERIALES", "Back to library": "Volver a la biblioteca", "Back to project": "Volver al proyecto",
    "Capture the build, a finished print, or an assembly detail.": "Captura el montaje, una impresión terminada o un detalle del ensamblaje.", "Cover": "Portada", "Create a collection from the Collections tab, then add this project here.": "Crea una colección desde la pestaña Colecciones y añade aquí este proyecto.",
    "Create the first tag below.": "Crea la primera etiqueta a continuación.", "Custom field": "Campo personalizado", "Delete photo": "Eliminar foto", "Edit estimate": "Editar estimación", "Edit item": "Editar elemento", "Edit photo": "Editar foto", "Edit step": "Editar paso",
    "Favorite project": "Proyecto favorito", "Included": "Incluido", "Instructions": "Instrucciones", "Layer": "Capa", "Leave workshop mode": "Salir del modo taller", "Linked photos": "Fotos vinculadas", "Linked project file": "Archivo del proyecto vinculado", "Loading project": "Cargando proyecto",
    "Manual": "Manual", "Material": "Material", "Material estimate": "Estimación de material", "Move photo down": "Mover foto abajo", "Move photo up": "Mover foto arriba", "Next": "Siguiente", "No": "No",
    "No assembly steps": "No hay pasos de montaje", "No assembly steps have been added yet.": "Todavía no se ha añadido ningún paso de montaje.", "No linked file": "Ningún archivo vinculado", "No parts have been added yet.": "Todavía no se ha añadido ninguna pieza.",
    "No priced material": "Ningún material con precio", "No saved material estimate yet.": "Todavía no hay ninguna estimación de material guardada.", "Not set": "Sin definir", "Nozzle": "Boquilla", "Opening 3D viewer…": "Abriendo el visor 3D…",
    "Ordered build instructions linked to parts and photos.": "Instrucciones de montaje ordenadas y vinculadas a piezas y fotos.", "PARTS FOR THIS STEP": "PIEZAS PARA ESTE PASO", "PRINT COST": "COSTE DE IMPRESIÓN", "PROJECT": "PROYECTO", "PROJECT HANDOFF": "TRASPASO DEL PROYECTO",
    "Part type": "Tipo de pieza", "Parts used": "Piezas utilizadas", "Photo details": "Detalles de la foto", "Plastic per piece (g)": "Plástico por pieza (g)", "Plate": "Placa", "Plate-specific previews need a connection to Volum Desktop. The saved full model remains available.": "Las vistas previas por placa necesitan conexión con Volum Desktop. El modelo completo guardado sigue disponible.",
    "Previous": "Anterior", "Printed": "Impresa", "Printed files and purchased parts for this build.": "Archivos impresos y piezas compradas para este montaje.", "Printer": "Impresora", "Profile": "Perfil", "Project details copied to the clipboard.": "Los detalles del proyecto se han copiado al portapapeles.",
    "Project file": "Archivo del proyecto", "Project summary": "Resumen del proyecto", "Purchase link": "Enlace de compra", "Purchased": "Comprada", "Quantity": "Cantidad", "Rename project": "Cambiar nombre del proyecto", "Renaming…": "Cambiando el nombre…", "Reset 3D view": "Restablecer vista 3D",
    "Save details, photos, and the 3D view to this device.": "Guarda los detalles, las fotos y la vista 3D en este dispositivo.", "Save estimate": "Guardar estimación", "Save item": "Guardar elemento", "Save name": "Guardar nombre", "Save photo": "Guardar foto", "Save step": "Guardar paso", "Save tags": "Guardar etiquetas", "Saving…": "Guardando…",
    "Scan from Library on another device paired with": "Escanéalo desde la Biblioteca de otro dispositivo vinculado con", "Scan to open": "Escanear para abrir", "Share project": "Compartir proyecto", "Show project QR code": "Mostrar código QR del proyecto", "Shown in the library and detail header.": "Se muestra en la biblioteca y en la cabecera de los detalles.",
    "Source page": "Página de origen", "Step title": "Título del paso", "TAG": "ETIQUETA", "Tap to include": "Toca para incluir", "The code contains project IDs only—never your token or file paths.": "El código solo contiene identificadores de proyecto, nunca el token ni rutas de archivos.",
    "Unit": "Unidad", "Update available": "Actualización disponible", "Use as project cover": "Usar como portada del proyecto", "View": "Ver", "WORKSHOP MODE": "MODO TALLER", "WORKSHOP PHOTO": "FOTO DE TALLER", "Weight source": "Origen del peso", "Whole project": "Todo el proyecto", "Yes": "Sí",
    "g each": "g cada una", "objects": "objetos", "parts": "piezas", "photos": "fotos", "this assembly step": "este paso de montaje", "triangles": "triángulos",
    "Delete {name}": "Eliminar {name}", "Delete “{name}” from the bill of materials?": "¿Eliminar «{name}» de la lista de materiales?", "Delete “{name}”?": "¿Eliminar «{name}»?", "Delete “{name}”? It will be removed from {count} projects.": "¿Eliminar «{name}»? Se eliminará de {count} proyectos.",
    "Drag to rotate · Pinch to zoom": "Arrastra para girar · Pellizca para ampliar", "Manage {name}": "Gestionar {name}", "Move {name} down": "Mover {name} abajo", "Move {name} up": "Mover {name} arriba", "Plate {number}": "Placa {number}", "Preparing 3D model…": "Preparando el modelo 3D…",
    "QR code for {name}": "Código QR de {name}", "Save {name}": "Guardar {name}", "Saved to {name}.": "Guardada en {name}.", "Step {number}": "Paso {number}", "This device was unpaired, but its offline cache could not be fully removed: {error}": "El dispositivo se ha desvinculado, pero no se pudo eliminar por completo la caché sin conexión: {error}",
    "Unpair this device and remove its cached credentials?": "¿Desvincular este dispositivo y eliminar sus credenciales guardadas?", "Used by {count} {projects}.": "Utilizada por {count} {projects}.", "View {name} in 3D": "Ver {name} en 3D", "WORKSHOP MODE · {current} OF {total}": "MODO TALLER · {current} DE {total}",
    "Enter valid device details.": "Introduce datos de dispositivo válidos.", "Create a new pairing code on Volum Desktop.": "Crea un código de vinculación nuevo en Volum Desktop.", "Volum could not save this paired device.": "Volum no pudo guardar este dispositivo vinculado.",
    "Pair this device with Volum Desktop.": "Vincula este dispositivo con Volum Desktop.", "This device is not paired or has been revoked.": "Este dispositivo no está vinculado o se ha revocado su acceso.", "The pairing code is not valid.": "El código de vinculación no es válido.",
    "Enable mobile organization changes in Volum Desktop settings first.": "Activa primero los cambios de organización móviles en los ajustes de Volum Desktop.", "This project changed on another device. Refresh and try again.": "Este proyecto ha cambiado en otro dispositivo. Actualízalo y vuelve a intentarlo.",
    "The requested Volum item is unavailable.": "El elemento de Volum solicitado no está disponible.", "The request is invalid.": "La solicitud no es válida.", "The requested change is invalid.": "El cambio solicitado no es válido.", "The selected upload is invalid.": "La subida seleccionada no es válida.", "The upload size is required.": "Se necesita el tamaño de la subida.",
    "Project photos must be non-empty and at most 25 MB.": "Las fotos de proyecto no pueden estar vacías y deben tener como máximo 25 MB.", "Another Companion upload is being imported. Try again shortly.": "Se está importando otra subida del Companion. Vuelve a intentarlo en breve.",
    "The model viewer is temporarily unavailable.": "El visor de modelos no está disponible temporalmente.", "This model is too detailed for the mobile viewer.": "Este modelo tiene demasiado detalle para el visor móvil.", "3D files can be at most 500 MB.": "Los archivos 3D pueden tener como máximo 500 MB.",
    "Volum Desktop could not import the uploaded file.": "Volum Desktop no pudo importar el archivo subido.", "Configure a managed library on Volum Desktop before uploading files.": "Configura una biblioteca gestionada en Volum Desktop antes de subir archivos.",
    "Choose a managed project that is not in Trash.": "Elige un proyecto gestionado que no esté en la Papelera.", "Volum Desktop could not finish saving this upload.": "Volum Desktop no pudo terminar de guardar esta subida.", "Volum could not complete this request.": "Volum no pudo completar esta solicitud.",
    "Volum Desktop returned an unreadable response": "Volum Desktop devolvió una respuesta ilegible", "Could not reach Volum Desktop through Tailscale.": "No se pudo contactar con Volum Desktop mediante Tailscale.", "Volum Desktop rejected this upload ({status})": "Volum Desktop rechazó esta subida ({status})", "Volum Desktop returned {status}": "Volum Desktop devolvió {status}",
    "This Volum Desktop identity changed during pairing.": "La identidad de este Volum Desktop cambió durante la vinculación.", "Volum Desktop returned an invalid media response.": "Volum Desktop devolvió una respuesta multimedia no válida.", "Volum Desktop returned an unreadable media response.": "Volum Desktop devolvió una respuesta multimedia ilegible.",
    "Enter your Volum Desktop Tailnet address.": "Introduce la dirección Tailnet de Volum Desktop.", "That Tailnet address is not valid.": "Esa dirección Tailnet no es válida.", "Enter only the Volum Desktop Tailnet address.": "Introduce solo la dirección Tailnet de Volum Desktop.",
    "That is not a Volum pairing code.": "Eso no es un código de vinculación de Volum.", "That is not a compatible Volum pairing code.": "Eso no es un código de vinculación de Volum compatible.", "This pairing code has expired. Create a new one on Volum Desktop.": "Este código de vinculación ha caducado. Crea uno nuevo en Volum Desktop.",
    "Choose a JPEG, PNG, or WebP photo.": "Elige una foto JPEG, PNG o WebP.", "The offline snapshot identity is invalid.": "La identidad de la instantánea sin conexión no es válida.", "This project needs more than the 250 MB offline limit.": "Este proyecto necesita más del límite sin conexión de 250 MB.",
    "This device does not have enough storage for the offline project.": "Este dispositivo no tiene suficiente espacio para el proyecto sin conexión.", "This device is low on storage. Free at least 50 MB before saving projects offline.": "Este dispositivo tiene poco espacio. Libera al menos 50 MB antes de guardar proyectos sin conexión.",
    "The project identity is invalid.": "La identidad del proyecto no es válida.", "This is not a Volum project code.": "Esto no es un código de proyecto de Volum.", "This project code is invalid.": "Este código de proyecto no es válido.", "This source link is invalid.": "Este enlace de origen no es válido.", "Volum can only open ordinary HTTP or HTTPS source links.": "Volum solo puede abrir enlaces HTTP o HTTPS normales.",
    "The viewer received an incomplete model.": "El visor recibió un modelo incompleto.", "The viewer received an unsupported model.": "El visor recibió un modelo no compatible.", "The viewer received invalid model geometry.": "El visor recibió una geometría de modelo no válida.", "The model contains no displayable geometry.": "El modelo no contiene ninguna geometría visible.", "The model has invalid geometry bounds.": "Los límites geométricos del modelo no son válidos.", "This model could not be displayed.": "No se pudo mostrar este modelo.",
    "Volum Desktop did not respond. Check that it is open and Companion Access is on.": "Volum Desktop no responde. Comprueba que esté abierto y que el Acceso complementario esté activado.", "The connection to Volum Desktop failed.": "La conexión con Volum Desktop falló.", "This device is no longer paired with Volum Desktop": "Este dispositivo ya no está vinculado con Volum Desktop",
    "Pair this device with Volum Desktop before uploading": "Vincula este dispositivo con Volum Desktop antes de subir archivos", "Choose an existing project or enter a name for a new project": "Elige un proyecto existente o introduce un nombre para un proyecto nuevo", "The selected file is no longer available": "El archivo seleccionado ya no está disponible",
    "Pair this device and choose a project before uploading": "Vincula este dispositivo y elige un proyecto antes de subir archivos", "Refresh the project and enter a shorter photo caption": "Actualiza el proyecto e introduce un pie de foto más corto", "The selected photo is no longer available": "La foto seleccionada ya no está disponible",
    "Choose a non-empty photo": "Elige una foto que no esté vacía", "Project photos can be at most 25 MB": "Las fotos de proyecto pueden tener como máximo 25 MB", "The upload has already finished": "La subida ya ha terminado", "That upload is already running": "Esa subida ya está en curso",
    "Choose a non-empty 3D file": "Elige un archivo 3D que no esté vacío", "The selected file could not be inspected": "No se pudo inspeccionar el archivo seleccionado", "Volum Desktop returned an unreadable upload response": "Volum Desktop devolvió una respuesta de subida ilegible",
    "Choose an STL, 3MF, OBJ, STEP, or STP file": "Elige un archivo STL, 3MF, OBJ, STEP o STP", "Choose a JPEG, PNG, or WebP photo": "Elige una foto JPEG, PNG o WebP", "The Tailnet address is invalid": "La dirección Tailnet no es válida", "Volum Companion only connects to Tailscale addresses": "Volum Companion solo se conecta a direcciones de Tailscale",
    "The file size is required before uploading.": "Se necesita el tamaño del archivo antes de subirlo.", "Choose a non-empty 3D file.": "Elige un archivo 3D que no esté vacío.", "Volum Desktop could not prepare this upload.": "Volum Desktop no pudo preparar esta subida.", "The selected project is invalid.": "El proyecto seleccionado no es válido.", "The selected project is unavailable.": "El proyecto seleccionado no está disponible.",
    "Enter a valid project name.": "Introduce un nombre de proyecto válido.", "Choose either an existing project or a name for a new project.": "Elige un proyecto existente o un nombre para un proyecto nuevo.", "Volum Desktop could not save this upload.": "Volum Desktop no pudo guardar esta subida.", "The upload was interrupted.": "La subida se interrumpió.", "The uploaded file is too large.": "El archivo subido es demasiado grande.",
    "The uploaded data did not match the declared file size.": "Los datos subidos no coinciden con el tamaño de archivo declarado.", "The upload ended before the complete file arrived.": "La subida terminó antes de que llegara el archivo completo.", "The filename is not valid.": "El nombre del archivo no es válido.", "Choose an STL, 3MF, OBJ, STEP, or STP file.": "Elige un archivo STL, 3MF, OBJ, STEP o STP.",
    "The photo filename is not valid.": "El nombre del archivo de foto no es válido.", "The photo size is required before uploading.": "Se necesita el tamaño de la foto antes de subirla.", "Photo captions may contain up to 500 characters.": "Los pies de foto pueden tener hasta 500 caracteres.",
    "ready": "listo", "pending": "pendiente", "failed": "con errores", "printable": "imprimible", "source": "origen", "reference": "referencia", "version": "versión", "duplicate": "duplicado", "geometry": "geometría", "manual": "manual", "3mf": "perfil 3MF", "web": "página de origen",
    "3MF has no model document": "El 3MF no contiene ningún documento de modelo", "Archive contains no supported model files": "El archivo no contiene ningún archivo de modelo compatible",
    "Remove from favorites": "Quitar de favoritos", "Add to favorites": "Añadir a favoritos", "Batch estimate": "Estimación del lote", "Estimated cost": "Coste estimado", "Protocol": "Protocolo", "Refreshing": "Actualizando", "Pull to refresh": "Arrastra para actualizar",
  },
};

const errorFallbacks: Record<string, string> = {
  unauthorized: "Pair this device with Volum Desktop.",
  invalid_device: "Enter valid device details.",
  pairing_expired: "Create a new pairing code on Volum Desktop.",
  pairing_failed: "Volum could not save this paired device.",
  organization_writes_disabled: "Enable mobile organization changes in Volum Desktop settings first.",
  edit_conflict: "This project changed on another device. Refresh and try again.",
  not_found: "The requested Volum item is unavailable.",
  invalid_request: "The request is invalid.",
  invalid_change: "The requested change is invalid.",
  invalid_upload: "The selected upload is invalid.",
  upload_size_required: "The upload size is required.",
  photo_too_large: "Project photos must be non-empty and at most 25 MB.",
  upload_busy: "Another Companion upload is being imported. Try again shortly.",
  viewer_unavailable: "The model viewer is temporarily unavailable.",
  viewer_mesh_too_large: "This model is too detailed for the mobile viewer.",
  upload_too_large: "3D files can be at most 500 MB.",
  upload_failed: "Volum Desktop could not import the uploaded file.",
  managed_library_required: "Configure a managed library on Volum Desktop before uploading files.",
  upload_target_unavailable: "Choose a managed project that is not in Trash.",
  upload_write_failed: "Volum Desktop could not finish saving this upload.",
  request_failed: "Volum could not complete this request.",
  invalid_response: "Volum Desktop returned an unreadable response",
  connection_failed: "Could not reach Volum Desktop through Tailscale.",
};

export const localizedErrorFallbackKeys = Object.values(errorFallbacks);

function preferredLanguage(): AppLanguage {
  const stored = typeof localStorage === "undefined" ? null : localStorage.getItem("volum-language");
  if (stored === "en" || stored === "ca" || stored === "es") return stored;
  const browser = typeof navigator === "undefined" ? "en" : navigator.language.toLowerCase();
  if (browser.startsWith("ca")) return "ca";
  if (browser.startsWith("es")) return "es";
  return "en";
}

let language = preferredLanguage();

export function t(value: string, parameters?: Record<string, string | number>) {
  const translated = language === "en" ? value : dictionaries[language][value] ?? value;
  if (!parameters) return translated;
  return translated.replace(/\{([A-Za-z0-9_]+)\}/g, (match, key: string) =>
    parameters[key] == null ? match : String(parameters[key]));
}

export function localizedError(reason: unknown) {
  const message = reason instanceof Error ? reason.message : String(reason);
  const direct = t(message);
  if (direct !== message || language === "en") return direct;
  const rejectedUpload = message.match(/^Volum Desktop rejected this upload \((\d+)\)$/);
  if (rejectedUpload) return t("Volum Desktop rejected this upload ({status})", { status: rejectedUpload[1] });
  const returnedStatus = message.match(/^Volum Desktop returned (\d+)$/);
  if (returnedStatus) return t("Volum Desktop returned {status}", { status: returnedStatus[1] });
  const code = typeof reason === "object" && reason && "code" in reason
    ? String((reason as { code?: unknown }).code ?? "")
    : "";
  return errorFallbacks[code] ? t(errorFallbacks[code]) : message;
}

export function locale() {
  return language === "ca" ? "ca-ES" : language === "es" ? "es-ES" : undefined;
}

export function setLanguage(next: AppLanguage) {
  language = next;
  if (typeof localStorage !== "undefined") localStorage.setItem("volum-language", next);
  if (typeof window !== "undefined") {
    window.dispatchEvent(new CustomEvent("volum-language-change", { detail: next }));
  }
}

export function missingTranslationKeys(next: Exclude<AppLanguage, "en">, values: Iterable<string>) {
  return [...new Set(values)].filter((value) => !dictionaries[next][value]).sort();
}

export function useLanguage() {
  const [current, setCurrent] = useState(language);
  useEffect(() => {
    const update = (event: Event) => setCurrent((event as CustomEvent<AppLanguage>).detail);
    window.addEventListener("volum-language-change", update);
    return () => window.removeEventListener("volum-language-change", update);
  }, []);
  return current;
}
