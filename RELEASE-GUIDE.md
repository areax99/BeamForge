# BeamForge Release Guide

[English](#english) · [Italiano](#italiano) · [Español](#español) · [Français](#français) · [Deutsch](#deutsch)

---

## English

This guide explains how to build BeamForge release archives for macOS, Windows, and Linux, then attach them to one GitHub release. Build on the target operating system whenever possible; cross-compiling can require additional tools and is not covered here.

### Before you start

- Push the source-code commit you intend to release to GitHub.
- Install Rust and the native tools required by the target operating system, as described below.
- The helper script `build-release.local.py` is intentionally excluded from Git. Keep a separate copy in the project root, next to `Cargo.toml`, on every build computer. It requires Python 3.11 or newer. Do not add the script or generated archives to the repository.
- Choose a release version and use it consistently. For example, the first beta could use the tag `v1.0.0-beta.1` and the title `BeamForge 1.0.0 Beta 1`.

The tag identifies the source commit for the release. If the code changes after tagging, push a new commit and create a new tag for the next version; do not reuse a published tag.

### Install Rust and build prerequisites

Install the stable Rust toolchain with rustup on each build computer. Rustup installs `rustc`, Cargo, and the `rustup` command.

#### macOS

1. Install Apple's Xcode Command Line Tools:

   ```sh
   xcode-select --install
   ```

2. Install Rust from Terminal:

   ```sh
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

3. Close and reopen Terminal, then check `rustc --version` and `cargo --version`. Xcode Command Line Tools provide the Apple compiler, SDK, `lipo`, `sips`, `iconutil`, and signing utilities used to make the app bundle.

#### Windows

1. Install the **Visual Studio Build Tools** from <https://visualstudio.microsoft.com/downloads/>. In the installer, select **Desktop development with C++**, including the MSVC toolchain and a Windows SDK.
2. Download and run `rustup-init.exe` from <https://rustup.rs>. Keep the default stable **MSVC** toolchain (`x86_64-pc-windows-msvc` on 64-bit Windows).
3. Open a new PowerShell window and check `rustc --version` and `cargo --version`. If the linker or resource compiler is not found, reopen the Build Tools installer and verify the C++ workload and Windows SDK are installed.

#### Linux

Install the native development packages first. On Ubuntu/Debian:

```sh
sudo apt update
sudo apt install build-essential pkg-config libudev-dev libgtk-3-dev
```

On Fedora:

```sh
sudo dnf install gcc pkgconf systemd-devel gtk3-devel
```

Install Rust from a terminal:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Open a new terminal and check `rustc --version` and `cargo --version`. The listed native packages provide the C toolchain, GTK3, udev/systemd headers, and `pkg-config` required by dependencies.

### Build the release archives

Run commands from the project root, the directory containing `Cargo.toml` and your local copy of `build-release.local.py`. The helper builds for the current host and writes archives to `release-archives/`, which is local and excluded from Git.

#### macOS

The script creates Apple Silicon, Intel, or Universal app archives. Universal includes both architectures:

```sh
python3 build-release.local.py --arch all
```

Or build just one:

```sh
python3 build-release.local.py --arch arm64
python3 build-release.local.py --arch x86_64
python3 build-release.local.py --arch universal
```

The helper installs missing Rust targets with rustup and uses `scripts/macos-app.sh` to bundle and sign the app locally. Use `--arch all` only on a Mac with Xcode Command Line Tools installed. The resulting app is not notarized by Apple.

#### Windows

On a Windows PC, open PowerShell in the project root:

```powershell
py -3 build-release.local.py
```

This builds for the PC's native architecture. If compilation appears idle, check Task Manager; Rust may still be compiling a dependency. On a computer with limited memory, reduce parallel build jobs:

```powershell
$env:CARGO_BUILD_JOBS = "2"
py -3 build-release.local.py
```

The native MSVC C++ build tools and resource compiler must be available to Rust.

#### Linux

On the Linux computer, after installing the native packages listed above:

```sh
python3 build-release.local.py
```

This builds for the native Linux target. The archive is not an installer; the destination computer must also provide the required system libraries. To install a local build for the current user instead of making a release archive, run `sh scripts/install-linux.sh`.

### Archive names

Archive names include the version from `Cargo.toml`, the beta channel, operating system, and architecture. Examples:

```text
BeamForge-1.0.0-beta-macos-arm64.zip
BeamForge-1.0.0-beta-macos-x86_64.zip
BeamForge-1.0.0-beta-macos-universal.zip
BeamForge-1.0.0-beta-windows-x86_64.zip
BeamForge-1.0.0-beta-linux-x86_64-gnu.tar.gz
```

The helper does not replace existing archives. Use `--force` only when you intentionally want to regenerate an archive. If you change the version or distribution channel, make sure archive names, Git tag, and release title agree.

### Verify the packages

1. Confirm each build finishes successfully and prints `Created:` with the archive path.
2. Check that the archive opens and launch the app on its target operating system; if possible, test it on another computer as well.
3. macOS builds are signed locally, not notarized by Apple, so users may need to authorize the app before opening it. Linux system libraries must be present on the destination computer.

### Upload the archives to GitHub

Attach every platform package to the **same release**; do not create a separate release for each operating system.

1. Open the GitHub repository and go to **Releases**.
2. If the release does not exist, select **Draft a new release**.
3. Enter the tag (for example, `v1.0.0-beta.1`) and title (`BeamForge 1.0.0 Beta 1`). To add an archive to an existing release, open it and select **Edit release**; do not create another tag.
4. Drag the `.zip` and `.tar.gz` archives into the release attachments area. They can be uploaded from their respective build computers.
5. Select **Set as a pre-release** for a beta.
6. Confirm all desired packages are listed, then select **Publish release**. If the release was already published, save your changes.

The files appear under the release's **Assets**. They are available to users with repository access; assets in a public repository are visible to everyone.

### Example release notes

```markdown
The first public beta of BeamForge, a desktop application for laser design, cutting, and engraving, with a workflow similar to LightBurn.

This is beta software: features may change, and bugs or compatibility issues may occur. Always check the preview and machine settings before running a laser job.

**Downloads**
- **macOS:** Choose Universal for most Macs, or download the Apple Silicon or Intel build separately.
- **Windows and Linux:** Download the archive for your operating system and architecture.

The macOS app is not notarized by Apple. macOS may ask you to authorize it before opening.
```

---

## Italiano

Questa guida spiega come creare gli archivi di rilascio di BeamForge per macOS, Windows e Linux e allegarli a un'unica release GitHub. Quando possibile, compila sul sistema operativo di destinazione: la compilazione incrociata può richiedere strumenti aggiuntivi e non è trattata qui.

### Prima di iniziare

- Pubblica su GitHub il commit del codice che vuoi distribuire.
- Installa Rust e gli strumenti nativi richiesti dal sistema operativo, come descritto sotto.
- Lo script `build-release.local.py` è escluso intenzionalmente da Git. Tienine una copia separata nella cartella principale del progetto, accanto a `Cargo.toml`, su ogni computer di compilazione. Richiede Python 3.11 o successivo. Non aggiungere lo script o gli archivi generati al repository.
- Scegli una versione e usala coerentemente. Per la prima beta, ad esempio, usa il tag `v1.0.0-beta.1` e il titolo `BeamForge 1.0.0 Beta 1`.

Il tag identifica il commit del codice a cui la release si riferisce. Se modifichi il codice dopo aver creato il tag, pubblica un nuovo commit e crea un nuovo tag per la versione successiva; non riutilizzare un tag già pubblicato.

### Installare Rust e i prerequisiti di compilazione

Installa la toolchain stabile di Rust con rustup su ogni computer di compilazione. Rustup installa `rustc`, Cargo e il comando `rustup`.

#### macOS

1. Installa gli Xcode Command Line Tools di Apple:

   ```sh
   xcode-select --install
   ```

2. Installa Rust dal Terminale:

   ```sh
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

3. Chiudi e riapri il Terminale, poi verifica con `rustc --version` e `cargo --version`. Gli Xcode Command Line Tools forniscono il compilatore Apple, l'SDK, `lipo`, `sips`, `iconutil` e gli strumenti di firma usati per creare il bundle dell'app.

#### Windows

1. Installa i **Visual Studio Build Tools** da <https://visualstudio.microsoft.com/downloads/>. Nel programma di installazione seleziona **Sviluppo desktop con C++**, includendo la toolchain MSVC e un Windows SDK.
2. Scarica ed esegui `rustup-init.exe` da <https://rustup.rs>. Mantieni la toolchain stabile **MSVC** predefinita (`x86_64-pc-windows-msvc` su Windows a 64 bit).
3. Apri una nuova finestra di PowerShell e verifica con `rustc --version` e `cargo --version`. Se non vengono trovati il linker o il compilatore di risorse, controlla nel programma di installazione dei Build Tools che siano presenti il carico di lavoro C++ e il Windows SDK.

#### Linux

Installa prima i pacchetti di sviluppo nativi. Su Ubuntu/Debian:

```sh
sudo apt update
sudo apt install build-essential pkg-config libudev-dev libgtk-3-dev
```

Su Fedora:

```sh
sudo dnf install gcc pkgconf systemd-devel gtk3-devel
```

Installa Rust dal terminale:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Apri un nuovo terminale e verifica con `rustc --version` e `cargo --version`. I pacchetti nativi elencati forniscono la toolchain C, GTK3, gli header udev/systemd e `pkg-config` richiesti dalle dipendenze.

### Compilare gli archivi di rilascio

Esegui i comandi dalla cartella principale del progetto, dove si trovano `Cargo.toml` e la tua copia locale di `build-release.local.py`. Lo script compila per il sistema corrente e salva gli archivi in `release-archives/`, una cartella locale esclusa da Git.

#### macOS

Lo script crea archivi dell'app per Apple Silicon, Intel o Universal. Universal include entrambe le architetture:

```sh
python3 build-release.local.py --arch all
```

Oppure compila un solo tipo:

```sh
python3 build-release.local.py --arch arm64
python3 build-release.local.py --arch x86_64
python3 build-release.local.py --arch universal
```

Lo script installa con rustup gli eventuali target Rust mancanti e usa `scripts/macos-app.sh` per creare il bundle e firmare l'app localmente. Usa `--arch all` solo su un Mac con gli Xcode Command Line Tools installati. L'app risultante non è autenticata da Apple.

#### Windows

Su un PC Windows, apri PowerShell nella cartella principale del progetto:

```powershell
py -3 build-release.local.py
```

La compilazione usa l'architettura nativa del PC. Se sembra ferma, controlla Gestione attività: Rust potrebbe essere ancora impegnato a compilare una dipendenza. Su un computer con poca memoria puoi limitare i processi di compilazione:

```powershell
$env:CARGO_BUILD_JOBS = "2"
py -3 build-release.local.py
```

Gli strumenti di compilazione C++ nativi MSVC e il compilatore di risorse devono essere disponibili a Rust.

#### Linux

Sul computer Linux, dopo aver installato i pacchetti nativi elencati sopra:

```sh
python3 build-release.local.py
```

La compilazione usa il target Linux nativo. L'archivio non è un installer e il computer di destinazione deve avere le librerie di sistema necessarie. Per installare localmente una compilazione per l'utente corrente invece di creare un archivio di rilascio, esegui `sh scripts/install-linux.sh`.

### Nomi degli archivi

I nomi includono la versione letta da `Cargo.toml`, il canale beta, il sistema operativo e l'architettura. Esempi:

```text
BeamForge-1.0.0-beta-macos-arm64.zip
BeamForge-1.0.0-beta-macos-x86_64.zip
BeamForge-1.0.0-beta-macos-universal.zip
BeamForge-1.0.0-beta-windows-x86_64.zip
BeamForge-1.0.0-beta-linux-x86_64-gnu.tar.gz
```

Lo script non sovrascrive gli archivi esistenti. Usa `--force` solo se vuoi rigenerare intenzionalmente un archivio. Se cambi versione o canale di distribuzione, assicurati che nomi degli archivi, tag Git e titolo della release siano coerenti.

### Verificare i pacchetti

1. Controlla che ogni compilazione termini senza errori e mostri `Created:` con il percorso dell'archivio.
2. Verifica che l'archivio si apra e avvia l'app sul sistema previsto; se possibile, provala anche su un altro computer.
3. Le build macOS sono firmate localmente, ma non autenticate da Apple: gli utenti potrebbero dover autorizzare l'apertura dell'app. Le librerie di sistema Linux devono essere presenti anche sul computer di destinazione.

### Caricare gli archivi su GitHub

Allega tutti i pacchetti alla **stessa release**: non serve creare una release separata per ogni sistema operativo.

1. Apri il repository GitHub e vai in **Releases**.
2. Se la release non esiste, scegli **Draft a new release**.
3. Inserisci il tag (ad esempio `v1.0.0-beta.1`) e il titolo (`BeamForge 1.0.0 Beta 1`). Per aggiungere un archivio a una release esistente, aprila e scegli **Edit release**; non creare un altro tag.
4. Trascina gli archivi `.zip` e `.tar.gz` nell'area degli allegati. Puoi caricarli dai rispettivi computer di compilazione.
5. Seleziona **Set as a pre-release** per indicare che si tratta di una beta.
6. Controlla che siano presenti tutti i pacchetti desiderati, poi scegli **Publish release**. Se la release era già pubblicata, salva le modifiche.

I file compaiono nella sezione **Assets** della release e sono disponibili agli utenti che hanno accesso al repository. In un repository pubblico gli archivi sono visibili a tutti.

### Note di esempio per la release

```markdown
La prima beta pubblica di BeamForge, un'applicazione desktop per progettazione, taglio e incisione laser, con un flusso di lavoro simile a LightBurn.

Questa è una versione beta: le funzionalità possono cambiare e possono verificarsi errori o problemi di compatibilità. Controlla sempre l'anteprima e le impostazioni della macchina prima di avviare un lavoro laser.

**Download**
- **macOS:** scegli Universal per la maggior parte dei Mac oppure scarica separatamente la build Apple Silicon o Intel.
- **Windows e Linux:** scarica l'archivio per il tuo sistema operativo e la tua architettura.

L'app macOS non è autenticata da Apple. macOS potrebbe chiederti di autorizzarla prima di aprirla.
```

---

## Español

Esta guía explica cómo crear los archivos de lanzamiento de BeamForge para macOS, Windows y Linux y adjuntarlos a una sola versión de GitHub. Siempre que sea posible, compila en el sistema operativo de destino; la compilación cruzada puede requerir herramientas adicionales y no se trata aquí.

### Antes de empezar

- Publica en GitHub el commit del código que quieras distribuir.
- Instala Rust y las herramientas nativas que requiera el sistema operativo, como se indica a continuación.
- El script auxiliar `build-release.local.py` está excluido intencionadamente de Git. Guarda una copia independiente en la raíz del proyecto, junto a `Cargo.toml`, en cada ordenador de compilación. Requiere Python 3.11 o posterior. No añadas el script ni los archivos generados al repositorio.
- Elige una versión y úsala de forma coherente. Por ejemplo, la primera beta puede usar la etiqueta `v1.0.0-beta.1` y el título `BeamForge 1.0.0 Beta 1`.

La etiqueta identifica el commit de código al que corresponde la versión. Si modificas el código después de crearla, publica un nuevo commit y crea una etiqueta nueva para la siguiente versión; no reutilices una etiqueta ya publicada.

### Instalar Rust y los requisitos de compilación

Instala la versión estable de Rust mediante rustup en cada ordenador de compilación. Rustup instala `rustc`, Cargo y el comando `rustup`.

#### macOS

1. Instala las herramientas de línea de comandos de Xcode de Apple:

   ```sh
   xcode-select --install
   ```

2. Instala Rust desde Terminal:

   ```sh
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

3. Cierra y vuelve a abrir Terminal; comprueba `rustc --version` y `cargo --version`. Las herramientas de línea de comandos de Xcode incluyen el compilador de Apple, el SDK, `lipo`, `sips`, `iconutil` y las utilidades de firma necesarias para crear el paquete de la aplicación.

#### Windows

1. Instala **Visual Studio Build Tools** desde <https://visualstudio.microsoft.com/downloads/>. En el instalador, selecciona **Desarrollo para el escritorio con C++**, incluida la cadena de herramientas MSVC y un SDK de Windows.
2. Descarga y ejecuta `rustup-init.exe` desde <https://rustup.rs>. Mantén la cadena estable **MSVC** predeterminada (`x86_64-pc-windows-msvc` en Windows de 64 bits).
3. Abre una nueva ventana de PowerShell y comprueba `rustc --version` y `cargo --version`. Si no se encuentra el enlazador o el compilador de recursos, verifica en el instalador de Build Tools que estén instalados la carga de trabajo de C++ y el SDK de Windows.

#### Linux

Instala primero los paquetes de desarrollo nativos. En Ubuntu/Debian:

```sh
sudo apt update
sudo apt install build-essential pkg-config libudev-dev libgtk-3-dev
```

En Fedora:

```sh
sudo dnf install gcc pkgconf systemd-devel gtk3-devel
```

Instala Rust desde una terminal:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Abre una terminal nueva y comprueba `rustc --version` y `cargo --version`. Los paquetes nativos proporcionan las herramientas C, GTK3, los encabezados de udev/systemd y `pkg-config` que necesitan las dependencias.

### Compilar los archivos de lanzamiento

Ejecuta los comandos desde la raíz del proyecto, donde están `Cargo.toml` y tu copia local de `build-release.local.py`. El script compila para el sistema actual y guarda los archivos en `release-archives/`, una carpeta local excluida de Git.

#### macOS

El script crea archivos de la aplicación para Apple Silicon, Intel o Universal. Universal incluye ambas arquitecturas:

```sh
python3 build-release.local.py --arch all
```

O compila solo una:

```sh
python3 build-release.local.py --arch arm64
python3 build-release.local.py --arch x86_64
python3 build-release.local.py --arch universal
```

El script instala con rustup los targets de Rust que falten y usa `scripts/macos-app.sh` para empaquetar y firmar localmente la aplicación. Usa `--arch all` solo en un Mac con las herramientas de línea de comandos de Xcode instaladas. Apple no notariza la aplicación resultante.

#### Windows

En un PC con Windows, abre PowerShell en la raíz del proyecto:

```powershell
py -3 build-release.local.py
```

Compila para la arquitectura nativa del PC. Si parece que la compilación se ha detenido, comprueba el Administrador de tareas: Rust quizá siga compilando una dependencia. En equipos con poca memoria, limita los procesos de compilación:

```powershell
$env:CARGO_BUILD_JOBS = "2"
py -3 build-release.local.py
```

Rust debe tener acceso a las herramientas nativas de compilación C++ de MSVC y al compilador de recursos.

#### Linux

En el ordenador Linux, después de instalar los paquetes nativos indicados:

```sh
python3 build-release.local.py
```

Compila para el target nativo de Linux. El archivo no es un instalador y el equipo de destino también debe tener las bibliotecas de sistema necesarias. Para instalar una compilación local para el usuario actual en lugar de crear un archivo de lanzamiento, ejecuta `sh scripts/install-linux.sh`.

### Nombres de los archivos

Los nombres incluyen la versión de `Cargo.toml`, el canal beta, el sistema operativo y la arquitectura. Ejemplos:

```text
BeamForge-1.0.0-beta-macos-arm64.zip
BeamForge-1.0.0-beta-macos-x86_64.zip
BeamForge-1.0.0-beta-macos-universal.zip
BeamForge-1.0.0-beta-windows-x86_64.zip
BeamForge-1.0.0-beta-linux-x86_64-gnu.tar.gz
```

El script no reemplaza archivos existentes. Usa `--force` solo si quieres regenerar uno intencionadamente. Si cambias la versión o el canal de distribución, comprueba que coincidan los nombres, la etiqueta de Git y el título de la versión.

### Verificar los paquetes

1. Comprueba que cada compilación termine correctamente y muestre `Created:` con la ruta del archivo.
2. Verifica que el paquete se pueda abrir e inicia la aplicación en el sistema previsto; si es posible, pruébala también en otro ordenador.
3. Las versiones de macOS se firman localmente, pero Apple no las notariza, por lo que quizá haya que autorizar la aplicación antes de abrirla. Las bibliotecas de sistema de Linux también deben estar instaladas en el equipo de destino.

### Subir los archivos a GitHub

Adjunta todos los paquetes de plataforma a la **misma versión**; no hace falta crear una versión por sistema operativo.

1. Abre el repositorio de GitHub y ve a **Releases**.
2. Si la versión aún no existe, selecciona **Draft a new release**.
3. Introduce la etiqueta (por ejemplo, `v1.0.0-beta.1`) y el título (`BeamForge 1.0.0 Beta 1`). Para añadir un archivo a una versión existente, ábrela y selecciona **Edit release**; no crees otra etiqueta.
4. Arrastra los archivos `.zip` y `.tar.gz` al área de adjuntos. Puedes subirlos desde sus respectivos ordenadores de compilación.
5. Selecciona **Set as a pre-release** para indicar que es una beta.
6. Comprueba que aparezcan todos los paquetes y selecciona **Publish release**. Si la versión ya estaba publicada, guarda los cambios.

Los archivos aparecen en **Assets** y están disponibles para quienes tengan acceso al repositorio. En un repositorio público, son visibles para todos.

### Ejemplo de notas de lanzamiento

```markdown
La primera beta pública de BeamForge, una aplicación de escritorio para diseño, corte y grabado láser, con un flujo de trabajo similar al de LightBurn.

Este software está en fase beta: las funciones pueden cambiar y pueden aparecer errores o problemas de compatibilidad. Comprueba siempre la vista previa y los ajustes de la máquina antes de iniciar un trabajo láser.

**Descargas**
- **macOS:** elige Universal para la mayoría de los Mac o descarga por separado la versión para Apple Silicon o Intel.
- **Windows y Linux:** descarga el archivo correspondiente a tu sistema operativo y arquitectura.

La aplicación para macOS no está notarizada por Apple. Es posible que macOS te pida autorizarla antes de abrirla.
```

---

## Français

Ce guide explique comment créer les archives de version de BeamForge pour macOS, Windows et Linux, puis les joindre à une seule version GitHub. Dans la mesure du possible, compilez sur le système d'exploitation cible ; la compilation croisée peut nécessiter des outils supplémentaires et n'est pas abordée ici.

### Avant de commencer

- Publiez sur GitHub le commit du code que vous souhaitez distribuer.
- Installez Rust et les outils natifs requis par le système d'exploitation, comme indiqué ci-dessous.
- Le script auxiliaire `build-release.local.py` est volontairement exclu de Git. Conservez une copie séparée à la racine du projet, à côté de `Cargo.toml`, sur chaque ordinateur de compilation. Il nécessite Python 3.11 ou une version ultérieure. N'ajoutez ni le script ni les archives générées au dépôt.
- Choisissez une version et utilisez-la de façon cohérente. Par exemple, la première bêta peut utiliser le tag `v1.0.0-beta.1` et le titre `BeamForge 1.0.0 Beta 1`.

Le tag identifie le commit de code associé à la version. Si le code change après la création du tag, publiez un nouveau commit et créez un nouveau tag pour la version suivante ; ne réutilisez pas un tag déjà publié.

### Installer Rust et les prérequis de compilation

Installez la version stable de Rust avec rustup sur chaque ordinateur de compilation. Rustup installe `rustc`, Cargo et la commande `rustup`.

#### macOS

1. Installez les outils de ligne de commande Xcode d'Apple :

   ```sh
   xcode-select --install
   ```

2. Installez Rust depuis Terminal :

   ```sh
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

3. Fermez puis rouvrez Terminal, et vérifiez `rustc --version` et `cargo --version`. Les outils de ligne de commande Xcode fournissent le compilateur Apple, le SDK, `lipo`, `sips`, `iconutil` et les utilitaires de signature nécessaires au bundle de l'application.

#### Windows

1. Installez les **Visual Studio Build Tools** depuis <https://visualstudio.microsoft.com/downloads/>. Dans l'installateur, sélectionnez **Développement Desktop en C++**, avec la chaîne MSVC et un SDK Windows.
2. Téléchargez et exécutez `rustup-init.exe` depuis <https://rustup.rs>. Gardez la chaîne stable **MSVC** par défaut (`x86_64-pc-windows-msvc` sur Windows 64 bits).
3. Ouvrez une nouvelle fenêtre PowerShell et vérifiez `rustc --version` et `cargo --version`. Si l'éditeur de liens ou le compilateur de ressources est introuvable, vérifiez dans l'installateur Build Tools que la charge de travail C++ et le SDK Windows sont installés.

#### Linux

Installez d'abord les paquets de développement natifs. Sur Ubuntu/Debian :

```sh
sudo apt update
sudo apt install build-essential pkg-config libudev-dev libgtk-3-dev
```

Sur Fedora :

```sh
sudo dnf install gcc pkgconf systemd-devel gtk3-devel
```

Installez Rust depuis un terminal :

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Ouvrez un nouveau terminal et vérifiez `rustc --version` et `cargo --version`. Les paquets natifs fournissent les outils C, GTK3, les en-têtes udev/systemd et `pkg-config` requis par les dépendances.

### Compiler les archives de version

Exécutez les commandes depuis la racine du projet, où se trouvent `Cargo.toml` et votre copie locale de `build-release.local.py`. Le script compile pour le système actuel et place les archives dans `release-archives/`, un dossier local exclu de Git.

#### macOS

Le script crée des archives de l'application pour Apple Silicon, Intel ou Universal. Universal inclut les deux architectures :

```sh
python3 build-release.local.py --arch all
```

Ou ne compilez qu'une seule variante :

```sh
python3 build-release.local.py --arch arm64
python3 build-release.local.py --arch x86_64
python3 build-release.local.py --arch universal
```

Le script ajoute avec rustup les cibles Rust manquantes et utilise `scripts/macos-app.sh` pour créer le bundle et signer l'application localement. Utilisez `--arch all` uniquement sur un Mac avec les outils de ligne de commande Xcode installés. Apple ne notarise pas l'application obtenue.

#### Windows

Sur un PC Windows, ouvrez PowerShell à la racine du projet :

```powershell
py -3 build-release.local.py
```

La compilation cible l'architecture native du PC. Si elle semble bloquée, vérifiez le Gestionnaire des tâches : Rust peut être en train de compiler une dépendance. Sur un ordinateur disposant de peu de mémoire, réduisez le nombre de tâches parallèles :

```powershell
$env:CARGO_BUILD_JOBS = "2"
py -3 build-release.local.py
```

Les outils natifs de compilation C++ MSVC et le compilateur de ressources doivent être accessibles à Rust.

#### Linux

Sur l'ordinateur Linux, après avoir installé les paquets natifs indiqués ci-dessus :

```sh
python3 build-release.local.py
```

La compilation cible le système Linux natif. L'archive n'est pas un installateur et l'ordinateur de destination doit également disposer des bibliothèques système requises. Pour installer une compilation locale pour l'utilisateur actuel au lieu de créer une archive de version, exécutez `sh scripts/install-linux.sh`.

### Noms des archives

Les noms comprennent la version de `Cargo.toml`, le canal bêta, le système d'exploitation et l'architecture. Exemples :

```text
BeamForge-1.0.0-beta-macos-arm64.zip
BeamForge-1.0.0-beta-macos-x86_64.zip
BeamForge-1.0.0-beta-macos-universal.zip
BeamForge-1.0.0-beta-windows-x86_64.zip
BeamForge-1.0.0-beta-linux-x86_64-gnu.tar.gz
```

Le script ne remplace pas les archives existantes. N'utilisez `--force` que pour en recréer une intentionnellement. Si vous modifiez la version ou le canal de distribution, vérifiez la cohérence des noms d'archives, du tag Git et du titre de la version.

### Vérifier les paquets

1. Vérifiez que chaque compilation se termine sans erreur et affiche `Created:` avec le chemin de l'archive.
2. Vérifiez que l'archive s'ouvre et lancez l'application sur le système prévu ; si possible, testez-la également sur un autre ordinateur.
3. Les versions macOS sont signées localement, mais non notarisées par Apple : les utilisateurs devront peut-être autoriser l'application avant de l'ouvrir. Les bibliothèques système Linux doivent aussi être présentes sur l'ordinateur de destination.

### Téléverser les archives sur GitHub

Joignez tous les paquets à la **même version** ; il n'est pas nécessaire de créer une version par système d'exploitation.

1. Ouvrez le dépôt GitHub et accédez à **Releases**.
2. Si la version n'existe pas, sélectionnez **Draft a new release**.
3. Saisissez le tag (par exemple `v1.0.0-beta.1`) et le titre (`BeamForge 1.0.0 Beta 1`). Pour ajouter une archive à une version existante, ouvrez-la et sélectionnez **Edit release** ; ne créez pas un autre tag.
4. Faites glisser les archives `.zip` et `.tar.gz` dans la zone des pièces jointes. Elles peuvent être téléversées depuis leurs ordinateurs de compilation respectifs.
5. Cochez **Set as a pre-release** pour indiquer qu'il s'agit d'une bêta.
6. Vérifiez que tous les paquets souhaités sont présents, puis sélectionnez **Publish release**. Si la version était déjà publiée, enregistrez les modifications.

Les fichiers apparaissent dans **Assets** et sont accessibles aux personnes autorisées à consulter le dépôt. Dans un dépôt public, ils sont visibles par tous.

### Exemple de notes de version

```markdown
La première bêta publique de BeamForge, une application de bureau pour la conception, la découpe et la gravure laser, avec un flux de travail similaire à LightBurn.

Ce logiciel est en version bêta : les fonctionnalités peuvent évoluer et des bogues ou problèmes de compatibilité peuvent survenir. Vérifiez toujours l'aperçu et les réglages de la machine avant de lancer un travail laser.

**Téléchargements**
- **macOS :** choisissez Universal pour la plupart des Mac, ou téléchargez séparément la version Apple Silicon ou Intel.
- **Windows et Linux :** téléchargez l'archive correspondant à votre système et à votre architecture.

L'application macOS n'est pas notariée par Apple. macOS peut demander une autorisation avant son ouverture.
```

---

## Deutsch

Diese Anleitung beschreibt, wie BeamForge-Release-Archive für macOS, Windows und Linux erstellt und an eine gemeinsame GitHub-Version angehängt werden. Wenn möglich, sollte auf dem jeweiligen Zielbetriebssystem gebaut werden; Cross-Compiling kann zusätzliche Werkzeuge erfordern und wird hier nicht behandelt.

### Vor dem Start

- Veröffentliche den GitHub-Commit mit dem Quellcode, den du bereitstellen möchtest.
- Installiere Rust und die für das Betriebssystem benötigten nativen Werkzeuge wie unten beschrieben.
- Das Hilfsskript `build-release.local.py` ist absichtlich von Git ausgeschlossen. Lege auf jedem Build-Rechner eine separate Kopie im Projekt-Hauptverzeichnis neben `Cargo.toml` ab. Es benötigt Python 3.11 oder neuer. Füge weder das Skript noch erzeugte Archive zum Repository hinzu.
- Lege eine Versionsnummer fest und verwende sie einheitlich. Für die erste Beta eignen sich beispielsweise der Tag `v1.0.0-beta.1` und der Titel `BeamForge 1.0.0 Beta 1`.

Der Tag verweist auf den Quellcode-Commit der Version. Wird der Code nach dem Tag geändert, veröffentliche einen neuen Commit und erstelle für die nächste Version einen neuen Tag. Einen bereits veröffentlichten Tag nicht wiederverwenden.

### Rust und Build-Voraussetzungen installieren

Installiere auf jedem Build-Rechner mit rustup die stabile Rust-Toolchain. Rustup installiert `rustc`, Cargo und den Befehl `rustup`.

#### macOS

1. Installiere Apples Xcode Command Line Tools:

   ```sh
   xcode-select --install
   ```

2. Installiere Rust im Terminal:

   ```sh
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

3. Schließe das Terminal und öffne es erneut. Prüfe `rustc --version` und `cargo --version`. Die Xcode Command Line Tools enthalten Apples Compiler, SDK, `lipo`, `sips`, `iconutil` und die Signaturwerkzeuge für das App-Bundle.

#### Windows

1. Installiere die **Visual Studio Build Tools** von <https://visualstudio.microsoft.com/downloads/>. Wähle im Installer **Desktopentwicklung mit C++** einschließlich MSVC-Toolchain und Windows SDK aus.
2. Lade `rustup-init.exe` von <https://rustup.rs> herunter und führe es aus. Behalte die standardmäßige stabile **MSVC**-Toolchain bei (`x86_64-pc-windows-msvc` bei 64-Bit-Windows).
3. Öffne ein neues PowerShell-Fenster und prüfe `rustc --version` sowie `cargo --version`. Falls Linker oder Ressourcencompiler fehlen, prüfe im Build-Tools-Installer, ob die C++-Arbeitslast und das Windows SDK installiert sind.

#### Linux

Installiere zuerst die nativen Entwicklungspakete. Unter Ubuntu/Debian:

```sh
sudo apt update
sudo apt install build-essential pkg-config libudev-dev libgtk-3-dev
```

Unter Fedora:

```sh
sudo dnf install gcc pkgconf systemd-devel gtk3-devel
```

Installiere Rust im Terminal:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Öffne ein neues Terminal und prüfe `rustc --version` und `cargo --version`. Die nativen Pakete enthalten die C-Werkzeuge, GTK3, udev/systemd-Header und `pkg-config`, die von Abhängigkeiten benötigt werden.

### Release-Archive erstellen

Führe die Befehle im Projekt-Hauptverzeichnis aus, in dem `Cargo.toml` und deine lokale Kopie von `build-release.local.py` liegen. Das Skript baut für das aktuelle Betriebssystem und legt Archive in `release-archives/` ab, einem lokalen, von Git ausgeschlossenen Verzeichnis.

#### macOS

Das Skript erstellt App-Archive für Apple Silicon, Intel oder Universal. Universal enthält beide Architekturen:

```sh
python3 build-release.local.py --arch all
```

Oder erstelle nur eine Variante:

```sh
python3 build-release.local.py --arch arm64
python3 build-release.local.py --arch x86_64
python3 build-release.local.py --arch universal
```

Das Skript ergänzt fehlende Rust-Targets mit rustup und verwendet `scripts/macos-app.sh`, um das App-Bundle zu erstellen und lokal zu signieren. Verwende `--arch all` nur auf einem Mac mit installierten Xcode Command Line Tools. Die erstellte App ist nicht von Apple notarisiert.

#### Windows

Öffne auf einem Windows-PC PowerShell im Projekt-Hauptverzeichnis:

```powershell
py -3 build-release.local.py
```

Es wird für die native Architektur des PCs gebaut. Scheint der Build festzustecken, prüfe den Task-Manager; Rust kompiliert möglicherweise noch eine Abhängigkeit. Bei wenig Arbeitsspeicher lässt sich die Zahl paralleler Build-Prozesse begrenzen:

```powershell
$env:CARGO_BUILD_JOBS = "2"
py -3 build-release.local.py
```

Die nativen MSVC-C++-Build-Werkzeuge und der Ressourcencompiler müssen für Rust verfügbar sein.

#### Linux

Führe auf dem Linux-Rechner nach der Installation der oben genannten nativen Pakete Folgendes aus:

```sh
python3 build-release.local.py
```

Es wird für das native Linux-Target gebaut. Das Archiv ist kein Installer; die benötigten Systembibliotheken müssen auch auf dem Zielrechner vorhanden sein. Um einen lokalen Build für den aktuellen Benutzer zu installieren, statt ein Release-Archiv zu erstellen, führe `sh scripts/install-linux.sh` aus.

### Archivnamen

Die Namen enthalten die Version aus `Cargo.toml`, den Beta-Kanal, das Betriebssystem und die Architektur. Beispiele:

```text
BeamForge-1.0.0-beta-macos-arm64.zip
BeamForge-1.0.0-beta-macos-x86_64.zip
BeamForge-1.0.0-beta-macos-universal.zip
BeamForge-1.0.0-beta-windows-x86_64.zip
BeamForge-1.0.0-beta-linux-x86_64-gnu.tar.gz
```

Das Skript überschreibt vorhandene Archive nicht. Verwende `--force` nur, wenn du ein Archiv bewusst neu erstellen möchtest. Bei Änderungen an Version oder Vertriebskanal müssen Archivnamen, Git-Tag und Release-Titel zusammenpassen.

### Pakete überprüfen

1. Prüfe, ob jeder Build ohne Fehler endet und `Created:` mit dem Archivpfad ausgibt.
2. Stelle sicher, dass sich das Archiv öffnen lässt, und starte die App auf dem vorgesehenen Betriebssystem. Teste sie nach Möglichkeit auch auf einem zweiten Rechner.
3. macOS-Builds werden lokal signiert, aber nicht von Apple notarisiert. Benutzer müssen das Öffnen eventuell erst erlauben. Die Linux-Systembibliotheken müssen auch auf dem Zielrechner vorhanden sein.

### Archive auf GitHub hochladen

Hänge alle Plattformpakete an **dieselbe Release-Version** an; für jedes Betriebssystem ist keine eigene Release erforderlich.

1. Öffne das GitHub-Repository und wechsle zu **Releases**.
2. Falls die Version noch nicht existiert, wähle **Draft a new release**.
3. Gib den Tag (zum Beispiel `v1.0.0-beta.1`) und den Titel (`BeamForge 1.0.0 Beta 1`) ein. Um einer bestehenden Version ein Archiv hinzuzufügen, öffne sie und wähle **Edit release**; erstelle keinen zweiten Tag.
4. Ziehe die `.zip`- und `.tar.gz`-Archive in den Anhangsbereich. Sie können von den jeweiligen Build-Rechnern hochgeladen werden.
5. Wähle **Set as a pre-release**, um die Version als Beta zu kennzeichnen.
6. Prüfe, ob alle gewünschten Pakete aufgeführt sind, und wähle **Publish release**. Bei einer bereits veröffentlichten Version speichere die Änderungen.

Die Dateien werden unter **Assets** angezeigt und sind für Personen mit Zugriff auf das Repository verfügbar. In einem öffentlichen Repository kann sie jeder herunterladen.

### Beispiel für Release-Hinweise

```markdown
Die erste öffentliche Beta von BeamForge, einer Desktop-Anwendung für Laserdesign, Schneiden und Gravieren mit einem ähnlichen Arbeitsablauf wie LightBurn.

Diese Software befindet sich in der Beta-Phase: Funktionen können sich ändern, und Fehler oder Kompatibilitätsprobleme können auftreten. Prüfe vor jedem Laserauftrag die Vorschau und die Maschineneinstellungen.

**Downloads**
- **macOS:** Für die meisten Macs Universal wählen oder die Apple-Silicon- bzw. Intel-Version separat herunterladen.
- **Windows und Linux:** Das Archiv für das jeweilige Betriebssystem und die Architektur herunterladen.

Die macOS-App ist nicht von Apple notarisiert. macOS fordert möglicherweise vor dem Öffnen eine Freigabe an.
```
