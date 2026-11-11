# BeamForge Build Guide

[English](#english) · [Italiano](#italiano) · [Español](#español) · [Français](#français) · [Deutsch](#deutsch)

---

## English

This guide explains how to compile BeamForge on macOS, Windows, and Linux. Build on the target operating system whenever possible; cross-compiling can require additional tools and is not covered here.

### Before you start

Install Rust and the native tools required by your operating system, as described below.

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

### Build BeamForge

Run commands from the project root, the directory containing `Cargo.toml`.

#### macOS

Create the app for this Mac:

```sh
sh scripts/macos-app.sh
```

To build for a specific architecture:

```sh
sh scripts/macos-app.sh arm
sh scripts/macos-app.sh intel
sh scripts/macos-app.sh universal
```

The app is created at `dist/BeamForge.app`. The script installs missing Rust targets with rustup and signs the app locally. Universal includes both Apple Silicon and Intel; it requires Xcode Command Line Tools. The resulting app is not notarized by Apple.

#### Windows

On a Windows PC, open PowerShell in the project root and run:

```powershell
cargo build --release
```

The executable is created at `target\release\beamforge.exe`. If compilation appears idle, check Task Manager; Rust may still be compiling a dependency. On a computer with limited memory, reduce parallel build jobs:

```powershell
$env:CARGO_BUILD_JOBS = "2"
cargo build --release
```

The native MSVC C++ build tools and resource compiler must be available to Rust.

#### Linux

On the Linux computer, after installing the native packages listed above, build the executable:

```sh
cargo build --release
```

The executable is created at `target/release/beamforge`. To install it for the current user with an application-menu entry, run `sh scripts/install-linux.sh`. The destination computer must also provide the required system libraries.

### Verify the build

Confirm that the build finishes without errors, then launch the app or executable on the target operating system. If possible, test it on another computer as well. macOS builds are signed locally, not notarized by Apple, so macOS may ask you to authorize the app before opening it.

---

## Italiano

Questa guida spiega come compilare BeamForge su macOS, Windows e Linux. Quando possibile, compila sul sistema operativo di destinazione: la compilazione incrociata può richiedere strumenti aggiuntivi e non è trattata qui.

### Prima di iniziare

Installa Rust e gli strumenti nativi richiesti dal sistema operativo, come descritto sotto.

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

### Compilare BeamForge

Esegui i comandi dalla cartella principale del progetto, dove si trova `Cargo.toml`.

#### macOS

Per creare l'app per questo Mac:

```sh
sh scripts/macos-app.sh
```

Per compilare per un'architettura specifica:

```sh
sh scripts/macos-app.sh arm
sh scripts/macos-app.sh intel
sh scripts/macos-app.sh universal
```

L'app viene creata in `dist/BeamForge.app`. Lo script installa con rustup gli eventuali target Rust mancanti e firma l'app localmente. Universal include Apple Silicon e Intel e richiede gli Xcode Command Line Tools. L'app risultante non è autenticata da Apple.

#### Windows

Su un PC Windows, apri PowerShell nella cartella principale del progetto ed esegui:

```powershell
cargo build --release
```

L'eseguibile viene creato in `target\release\beamforge.exe`. Se sembra ferma, controlla Gestione attività: Rust potrebbe essere ancora impegnato a compilare una dipendenza. Su un computer con poca memoria puoi limitare i processi di compilazione:

```powershell
$env:CARGO_BUILD_JOBS = "2"
cargo build --release
```

Gli strumenti di compilazione C++ nativi MSVC e il compilatore di risorse devono essere disponibili a Rust.

#### Linux

Sul computer Linux, dopo aver installato i pacchetti nativi elencati sopra, compila l'eseguibile:

```sh
cargo build --release
```

L'eseguibile viene creato in `target/release/beamforge`. Per installarlo per l'utente corrente e aggiungere una voce al menu delle applicazioni, esegui `sh scripts/install-linux.sh`. Il computer di destinazione deve avere anche le librerie di sistema necessarie.

### Verificare la compilazione

Controlla che la compilazione termini senza errori e avvia l'app o l'eseguibile sul sistema operativo previsto. Se possibile, provalo anche su un altro computer. Le build macOS non sono autenticate da Apple: potrebbe essere necessario autorizzare l'apertura dell'app.

---

## Español

Esta guía explica cómo compilar BeamForge en macOS, Windows y Linux. Siempre que sea posible, compila en el sistema operativo de destino; la compilación cruzada puede requerir herramientas adicionales y no se trata aquí.

### Antes de empezar

Instala Rust y las herramientas nativas que requiera el sistema operativo, como se indica a continuación.

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

### Compilar BeamForge

Ejecuta los comandos desde la raíz del proyecto, donde está `Cargo.toml`.

#### macOS

Para crear la aplicación para este Mac:

```sh
sh scripts/macos-app.sh
```

Para compilar para una arquitectura concreta:

```sh
sh scripts/macos-app.sh arm
sh scripts/macos-app.sh intel
sh scripts/macos-app.sh universal
```

La aplicación se crea en `dist/BeamForge.app`. El script instala con rustup los targets de Rust que falten y firma la aplicación localmente. Universal incluye Apple Silicon e Intel y requiere las herramientas de línea de comandos de Xcode. Apple no notariza la aplicación resultante.

#### Windows

En un PC con Windows, abre PowerShell en la raíz del proyecto y ejecuta:

```powershell
cargo build --release
```

El ejecutable se crea en `target\release\beamforge.exe`. Si parece que la compilación se ha detenido, comprueba el Administrador de tareas: Rust quizá siga compilando una dependencia. En equipos con poca memoria, limita los procesos de compilación:

```powershell
$env:CARGO_BUILD_JOBS = "2"
cargo build --release
```

Rust debe tener acceso a las herramientas nativas de compilación C++ de MSVC y al compilador de recursos.

#### Linux

En el ordenador Linux, después de instalar los paquetes nativos indicados, compila el ejecutable:

```sh
cargo build --release
```

El ejecutable se crea en `target/release/beamforge`. Para instalarlo para el usuario actual y añadir una entrada al menú de aplicaciones, ejecuta `sh scripts/install-linux.sh`. El equipo de destino también debe tener las bibliotecas de sistema necesarias.

### Verificar la compilación

Comprueba que la compilación termine sin errores y ejecuta la aplicación o el archivo ejecutable en el sistema operativo previsto. Si es posible, pruébalo también en otro ordenador. Apple no notariza las compilaciones de macOS, por lo que quizá tengas que autorizar la aplicación antes de abrirla.

---

## Français

Ce guide explique comment compiler BeamForge sur macOS, Windows et Linux. Dans la mesure du possible, compilez sur le système d'exploitation cible ; la compilation croisée peut nécessiter des outils supplémentaires et n'est pas abordée ici.

### Avant de commencer

Installez Rust et les outils natifs requis par le système d'exploitation, comme indiqué ci-dessous.

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

### Compiler BeamForge

Exécutez les commandes depuis la racine du projet, où se trouve `Cargo.toml`.

#### macOS

Pour créer l'application pour ce Mac :

```sh
sh scripts/macos-app.sh
```

Pour compiler pour une architecture précise :

```sh
sh scripts/macos-app.sh arm
sh scripts/macos-app.sh intel
sh scripts/macos-app.sh universal
```

L'application est créée dans `dist/BeamForge.app`. Le script ajoute avec rustup les cibles Rust manquantes et signe l'application localement. Universal inclut Apple Silicon et Intel et nécessite les outils de ligne de commande Xcode. Apple ne notarie pas l'application obtenue.

#### Windows

Sur un PC Windows, ouvrez PowerShell à la racine du projet et exécutez :

```powershell
cargo build --release
```

L'exécutable est créé dans `target\release\beamforge.exe`. Si la compilation semble bloquée, vérifiez le Gestionnaire des tâches : Rust peut être en train de compiler une dépendance. Sur un ordinateur disposant de peu de mémoire, réduisez le nombre de tâches parallèles :

```powershell
$env:CARGO_BUILD_JOBS = "2"
cargo build --release
```

Les outils natifs de compilation C++ MSVC et le compilateur de ressources doivent être accessibles à Rust.

#### Linux

Sur l'ordinateur Linux, après avoir installé les paquets natifs indiqués ci-dessus, compilez l'exécutable :

```sh
cargo build --release
```

L'exécutable est créé dans `target/release/beamforge`. Pour l'installer pour l'utilisateur actuel et ajouter une entrée au menu des applications, exécutez `sh scripts/install-linux.sh`. L'ordinateur doit également disposer des bibliothèques système requises.

### Vérifier la compilation

Vérifiez que la compilation se termine sans erreur, puis lancez l'application ou l'exécutable sur le système d'exploitation prévu. Si possible, testez-le également sur un autre ordinateur. Apple ne notarie pas les compilations macOS ; il peut donc être nécessaire d'autoriser l'application avant de l'ouvrir.

---

## Deutsch

Diese Anleitung beschreibt, wie BeamForge unter macOS, Windows und Linux kompiliert wird. Wenn möglich, sollte auf dem jeweiligen Zielbetriebssystem gebaut werden; Cross-Compiling kann zusätzliche Werkzeuge erfordern und wird hier nicht behandelt.

### Vor dem Start

Installiere Rust und die für das Betriebssystem benötigten nativen Werkzeuge wie unten beschrieben.

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

### BeamForge kompilieren

Führe die Befehle im Projekt-Hauptverzeichnis aus, in dem `Cargo.toml` liegt.

#### macOS

Erstelle die App für diesen Mac:

```sh
sh scripts/macos-app.sh
```

Für eine bestimmte Architektur:

```sh
sh scripts/macos-app.sh arm
sh scripts/macos-app.sh intel
sh scripts/macos-app.sh universal
```

Die App wird unter `dist/BeamForge.app` erstellt. Das Skript ergänzt fehlende Rust-Targets mit rustup und signiert die App lokal. Universal enthält Apple Silicon und Intel und benötigt installierte Xcode Command Line Tools. Die erstellte App ist nicht von Apple notarisiert.

#### Windows

Öffne auf einem Windows-PC PowerShell im Projekt-Hauptverzeichnis und führe Folgendes aus:

```powershell
cargo build --release
```

Die ausführbare Datei wird unter `target\release\beamforge.exe` erstellt. Scheint der Build festzustecken, prüfe den Task-Manager; Rust kompiliert möglicherweise noch eine Abhängigkeit. Bei wenig Arbeitsspeicher lässt sich die Zahl paralleler Build-Prozesse begrenzen:

```powershell
$env:CARGO_BUILD_JOBS = "2"
cargo build --release
```

Die nativen MSVC-C++-Build-Werkzeuge und der Ressourcencompiler müssen für Rust verfügbar sein.

#### Linux

Führe auf dem Linux-Rechner nach der Installation der oben genannten nativen Pakete folgenden Befehl aus:

```sh
cargo build --release
```

Die ausführbare Datei wird unter `target/release/beamforge` erstellt. Um sie für den aktuellen Benutzer zu installieren und einen Eintrag im Anwendungsmenü hinzuzufügen, führe `sh scripts/install-linux.sh` aus. Die benötigten Systembibliotheken müssen auch auf dem Zielrechner vorhanden sein.

### Build überprüfen

Prüfe, ob der Build ohne Fehler abgeschlossen wurde, und starte anschließend die App oder ausführbare Datei auf dem vorgesehenen Betriebssystem. Teste sie nach Möglichkeit auch auf einem zweiten Rechner. macOS-Builds sind nicht von Apple notarisiert; vor dem Öffnen muss die App möglicherweise freigegeben werden.
