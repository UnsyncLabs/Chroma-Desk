# chroma-desk

English · [Español](#español)

A modern desktop UI for [OpenRGB](https://openrgb.org) (Tauri 2 + Svelte 5 + Rust). It talks to the OpenRGB SDK server, so every device OpenRGB supports works.

## Features

- **Physical keyboard mock-up**: real key widths (ANSI/ISO, TKL/60 %/full), extras row (media keys, dial, logo), matrix fallback.
- **Paint any device LED by LED**: keyboards, matrices, strips, fan rings (pick a fan preset), single LEDs, multi-zone boards. Brush, rectangle selection, eyedropper, fill, eraser, quick groups (WASD, arrows, F-keys…), undo/redo (Ctrl+Z / Ctrl+Shift+Z).
- **Animations**: a library of reusable animations. Pick a ready-made preset (Rainbow, Breathing, Starlight, Wave, Chase, Alert, Heartbeat, Snake, Soft fire) with one click on the *Quick* tab, or draw your own in the *Draw* tab: **Frames** (a flipbook with timeline, onion skin, crossfade, loop/ping-pong/once), **Path** (drag over LEDs and a gradient travels along the route) and **Palette** (paint a shape with a gradient that rotates). Save them to the library, duplicate, rename, export and import `.json` files. Animations refer to LEDs by name (`Key: W`), so they also work on other keyboards (the UI shows how many LEDs match, e.g. `112/114 LEDs`).
- **Triggers**: make the lights react to anything (see [Triggers](#triggers)).
- **Effect layers** with opacity, blend modes (normal/add/multiply/screen) and masks: Solid, Paint, Gradient, Breathing, Spectrum, Rainbow wave, Colour wave, Starlight, Strobe, Chase, Ripple.
- **Global synchronisation**: one effect runs across all your devices in the order you choose (keyboard → fans → RAM).
- **Hardware mode**: native OpenRGB modes (speed, brightness, direction, colours, save to device).
- **Profiles**: save, apply, duplicate, rename, delete, default profile, import/export `.json`, import OpenRGB profiles. Zone sizes (e.g. fans on a Corsair Lighting Node) are restored automatically.
- **Background**: tray icon (Show, Profiles, Lights off, Quit), close-to-tray, single instance, start on login, `--hidden`, `--apply-profile <name>`.
- English and Spanish UI (add a language by adding a JSON file in `src/lib/i18n/`).

Not included: effects that react to key presses.

## Install

### Arch Linux

```sh
cd packaging/arch && makepkg -si
```

Depends on `openrgb` (or `openrgb-razer`), `webkit2gtk-4.1` and `libayatana-appindicator`.

### From source

```sh
npm ci
npm run tauri build -- --no-bundle   # binary in src-tauri/target/release/chroma-desk
```

Development: `npm run tauri dev`. UI only, without hardware: `MOCK=1 npm run dev` (in-memory fake backend, open the printed URL in a browser).

## Command line

```sh
chroma-desk                       # open the window
chroma-desk --hidden              # start in the tray
chroma-desk --apply-profile Name  # apply a profile and exit if it only uses hardware modes;
                                  # profiles with effects keep running in the tray
```

If chroma-desk is already running, a second launch focuses it (or applies the profile there). `chroma-desk trigger ...` never opens a window: it talks to the running instance (see [Triggers](#triggers)).

## Triggers

A trigger plays an animation **over** your current lights for a few seconds (an overlay) and then your scene comes back. Use it for notifications: a script, a game mod, a cron job, a build finishing. Overlays stack, have a priority (higher is drawn on top), can be limited to some keys, and at most 32 can be active. Only devices in *Effects* mode take part (devices running a native hardware mode are left alone). chroma-desk must be running (in the tray is fine).

### Command line

```sh
chroma-desk trigger "Alert"                                   # the whole keyboard, 3 s
chroma-desk trigger "Alert" --keys W,A,S,D --duration 2s      # only WASD, 2 seconds
chroma-desk trigger "Heartbeat" --device razer --repeat 3     # 3 passes of the animation, only devices matching "razer"
chroma-desk trigger "Alert" --color '#00aaff' --priority 5    # recoloured, drawn over lower priorities
chroma-desk trigger --stop                                    # stop every active overlay
```

Options: `--device <key or part of the name>`, `--keys <W,A,S,D | "Left Shift" | idx:41>`, `--duration <3s | 500ms | 1.5 | 2m>`, `--repeat <N passes>`, `--color <#rrggbb>`, `--priority <-1000..1000>`. The animation is a name or id from the library (built-in presets included, e.g. `Alert`, `Heartbeat`). Exit code 0 on success, 1 if the app is not running or refused the request, 2 for a bad command line.

### Local socket (scripts, game mods)

chroma-desk listens on `$XDG_RUNTIME_DIR/chroma-desk.sock`, a Unix socket that only your user can open (mode 0600; nothing listens on the network). Send one JSON object per line, get one JSON line back:

```json
{"cmd": "trigger", "animation": "Alert", "keys": ["W", "A", "S", "D"], "duration": "2s", "color": "#ff0000", "repeat": 2, "priority": 5, "device": "razer"}
{"cmd": "stop"}
{"cmd": "list"}
{"cmd": "ping"}
```

`cmd` defaults to `trigger`. `duration` is a number of seconds or text such as `"500ms"`. Replies: `{"ok": true, "overlay": 3, "devices": 1, "duration": 2.0}` or `{"ok": false, "error": "anim_not_found", "detail": "Nope"}`. Lines are limited to 8 KiB, unknown fields are rejected.

**Bash**

```sh
SOCK="$XDG_RUNTIME_DIR/chroma-desk.sock"
echo '{"animation":"Alert","keys":["W","A","S","D"],"duration":"2s"}' | socat - UNIX-CONNECT:"$SOCK"
echo '{"cmd":"stop"}' | nc -U "$SOCK"          # OpenBSD netcat works too

# e.g. flash red when a long command finishes
make && chroma-desk trigger "Heartbeat" --color '#00ff66' --duration 2s || chroma-desk trigger "Alert" --duration 3s
```

**Python**

```python
import json, os, socket

def trigger(**spec):
    path = os.path.join(os.environ["XDG_RUNTIME_DIR"], "chroma-desk.sock")
    with socket.socket(socket.AF_UNIX) as s:
        s.settimeout(5)
        s.connect(path)
        s.sendall((json.dumps({"cmd": "trigger", **spec}) + "\n").encode())
        return json.loads(s.makefile().readline())

print(trigger(animation="Alert", keys=["W", "A", "S", "D"], duration="2s"))
```

The *Triggers* section of Settings shows these commands ready to copy, and every animation card has *Copy trigger command*.

## Testing

```sh
cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings
npm run check && npm test
npm install --prefix e2e && npm run e2e   # real simulated mouse/keyboard against the MOCK=1 UI (needs Chromium)
```

## Start on login

Settings → *Start on login* creates `~/.config/autostart/chroma-desk.desktop` (runs `chroma-desk --hidden`). On Hyprland with uwsm, `systemd-xdg-autostart-generator` runs these entries; if yours does not, add `chroma-desk --hidden` to your compositor's autostart.

To keep the OpenRGB server alive, enable *Keep OpenRGB running with a systemd user service* in Settings (installs `chroma-desk-openrgb.service`, `openrgb --server --startminimized`). Close any OpenRGB you started by hand first.

## Troubleshooting

- **“Cannot reach OpenRGB”**: chroma-desk starts `openrgb --server --startminimized` if nothing listens on port 6742. Check that `openrgb` is installed and in `PATH`, or run it yourself with `--server`.
- **No devices / RAM or motherboard missing**: load the `i2c-dev` module (`echo i2c-dev | sudo tee /etc/modules-load.d/i2c.conf`) and install OpenRGB's udev rules (`60-openrgb.rules`), then re-login and press *Rescan*.
- **Two apps fighting over the lights**: the OpenRGB window and chroma-desk both write to devices; the last writer wins. chroma-desk warns when an OpenRGB window is open. Start OpenRGB with `--startminimized`.
- **Fan LEDs missing after reboot**: OpenRGB forgets zone sizes. chroma-desk re-applies the sizes you set in *Advanced*; make sure it starts on login or the default profile is set.
- **No tray icon**: you need a StatusNotifier host (Waybar `tray` module, KDE, GNOME extension). Without one, closing the window minimises it.

Config lives in `~/.config/chroma-desk/` (`config.json`, `profiles/*.json`, `animations/*.json`).

## License

GPL-2.0-or-later. Includes a patched copy of the `openrgb2` crate (GPL-2.0) in `src-tauri/vendor/`.

---

# Español

Una interfaz de escritorio moderna para [OpenRGB](https://openrgb.org) (Tauri 2 + Svelte 5 + Rust). Se comunica con el servidor SDK de OpenRGB, así que funciona con todos los dispositivos que OpenRGB soporte.

## Funciones

- **Maqueta física del teclado**: anchos reales de tecla (ANSI/ISO, TKL/60 %/completo), fila de extras (multimedia, dial, logo) y vista de matriz de respaldo.
- **Pintar cualquier dispositivo LED a LED**: teclados, matrices, tiras, anillos de ventilador (con preset), LEDs únicos y placas multizona. Pincel, selección por rectángulo, cuentagotas, relleno, borrador, grupos rápidos (WASD, flechas, F1–F12…) y deshacer/rehacer (Ctrl+Z / Ctrl+Shift+Z).
- **Animaciones**: una biblioteca de animaciones reutilizables. Elige un preset listo (Arcoíris, Respiración, Estrellas, Ola, Persecución, Alerta, Latido, Serpiente, Fuego suave) con un clic en la pestaña *Rápido*, o dibuja la tuya en *Dibujar*: **Fotogramas** (flipbook con línea de tiempo, papel cebolla, fundido y bucle/ida y vuelta/una vez), **Camino** (arrastra sobre los LEDs y un degradado viaja por la ruta) y **Paleta** (pinta una forma con un degradado que gira). Guárdalas en la biblioteca, duplícalas, renómbralas, expórtalas e impórtalas como `.json`. Las animaciones nombran los LEDs (`Key: W`), así que también funcionan en otros teclados (la interfaz muestra cuántos LEDs coinciden, p. ej. `112/114 LEDs`).
- **Disparadores**: haz que las luces reaccionen a lo que quieras (ver [Disparadores](#disparadores)).
- **Capas de efectos** con opacidad, modos de mezcla (normal/sumar/multiplicar/trama) y máscaras: Color sólido, Pintura, Degradado, Respiración, Espectro, Onda arcoíris, Onda de color, Estrellas, Estroboscopio, Persecución y Ondas.
- **Sincronización global**: un efecto recorre todos tus dispositivos en el orden que elijas (teclado → ventiladores → RAM).
- **Modo hardware**: modos nativos de OpenRGB (velocidad, brillo, dirección, colores, guardar en el dispositivo).
- **Perfiles**: guardar, aplicar, duplicar, renombrar, borrar, perfil predeterminado, importar/exportar `.json` e importar perfiles de OpenRGB. Los tamaños de zona (p. ej. ventiladores de un Corsair Lighting Node) se restauran solos.
- **Segundo plano**: icono de bandeja (Mostrar, Perfiles, Apagar todo, Salir), cerrar a la bandeja, instancia única, inicio con la sesión, `--hidden` y `--apply-profile <nombre>`.
- Interfaz en inglés y español (añadir un idioma = añadir un JSON en `src/lib/i18n/`).

No incluido: efectos reactivos a las teclas pulsadas.

## Instalación

### Arch Linux

```sh
cd packaging/arch && makepkg -si
```

Depende de `openrgb` (o `openrgb-razer`), `webkit2gtk-4.1` y `libayatana-appindicator`.

### Desde el código fuente

```sh
npm ci
npm run tauri build -- --no-bundle   # binario en src-tauri/target/release/chroma-desk
```

Desarrollo: `npm run tauri dev`. Solo la interfaz, sin hardware: `MOCK=1 npm run dev` (backend falso en memoria; abre la URL en un navegador).

## Línea de comandos

```sh
chroma-desk                       # abre la ventana
chroma-desk --hidden              # arranca en la bandeja
chroma-desk --apply-profile Nombre  # aplica un perfil y sale si solo usa modos hardware;
                                    # los perfiles con efectos siguen en la bandeja
```

Si chroma-desk ya está en marcha, un segundo lanzamiento lo enfoca (o aplica el perfil ahí). `chroma-desk trigger ...` nunca abre una ventana: habla con la instancia en marcha (ver [Disparadores](#disparadores)).

## Disparadores

Un disparador reproduce una animación **encima** de tus luces actuales durante unos segundos (una superposición) y luego vuelve tu escena. Úsalo para notificaciones: un script, un mod de juego, una tarea cron, un compilado que termina. Las superposiciones se apilan, tienen prioridad (la mayor se dibuja encima), pueden limitarse a algunas teclas y hay un máximo de 32 activas. Solo participan los dispositivos en modo *Efectos* (los que usan un modo nativo de hardware no se tocan). chroma-desk debe estar en marcha (en la bandeja basta).

### Línea de comandos

```sh
chroma-desk trigger "Alerta"                                  # todo el teclado, 3 s
chroma-desk trigger "Alerta" --keys W,A,S,D --duration 2s     # solo WASD, 2 segundos
chroma-desk trigger "Latido" --device razer --repeat 3        # 3 pasadas, solo dispositivos que coincidan con «razer»
chroma-desk trigger "Alerta" --color '#00aaff' --priority 5   # recoloreada, encima de prioridades menores
chroma-desk trigger --stop                                    # detiene todas las superposiciones
```

Opciones: `--device <clave o parte del nombre>`, `--keys <W,A,S,D | "Left Shift" | idx:41>`, `--duration <3s | 500ms | 1.5 | 2m>`, `--repeat <N pasadas>`, `--color <#rrggbb>`, `--priority <-1000..1000>`. La animación es un nombre o id de la biblioteca (incluidos los presets, p. ej. `Alert`, `Heartbeat`). Código de salida 0 si todo va bien, 1 si la app no está en marcha o rechazó la petición, 2 si la línea de comandos es incorrecta.

### Socket local (scripts, mods de juegos)

chroma-desk escucha en `$XDG_RUNTIME_DIR/chroma-desk.sock`, un socket Unix que solo tu usuario puede abrir (modo 0600; nada escucha en la red). Envía un objeto JSON por línea y recibes una línea JSON:

```json
{"cmd": "trigger", "animation": "Alert", "keys": ["W", "A", "S", "D"], "duration": "2s", "color": "#ff0000", "repeat": 2, "priority": 5, "device": "razer"}
{"cmd": "stop"}
{"cmd": "list"}
{"cmd": "ping"}
```

`cmd` es `trigger` por defecto. `duration` es un número de segundos o texto como `"500ms"`. Respuestas: `{"ok": true, "overlay": 3, "devices": 1, "duration": 2.0}` o `{"ok": false, "error": "anim_not_found", "detail": "Nope"}`. Las líneas se limitan a 8 KiB y los campos desconocidos se rechazan.

**Bash**

```sh
SOCK="$XDG_RUNTIME_DIR/chroma-desk.sock"
echo '{"animation":"Alert","keys":["W","A","S","D"],"duration":"2s"}' | socat - UNIX-CONNECT:"$SOCK"
echo '{"cmd":"stop"}' | nc -U "$SOCK"          # también sirve el netcat de OpenBSD

# p. ej. avisar cuando termina un comando largo
make && chroma-desk trigger "Heartbeat" --color '#00ff66' --duration 2s || chroma-desk trigger "Alert" --duration 3s
```

**Python**

```python
import json, os, socket

def trigger(**spec):
    path = os.path.join(os.environ["XDG_RUNTIME_DIR"], "chroma-desk.sock")
    with socket.socket(socket.AF_UNIX) as s:
        s.settimeout(5)
        s.connect(path)
        s.sendall((json.dumps({"cmd": "trigger", **spec}) + "\n").encode())
        return json.loads(s.makefile().readline())

print(trigger(animation="Alert", keys=["W", "A", "S", "D"], duration="2s"))
```

La sección *Disparadores* de Ajustes muestra estos comandos listos para copiar y cada tarjeta de animación tiene *Copiar comando de disparo*.

## Pruebas

```sh
cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings
npm run check && npm test
npm install --prefix e2e && npm run e2e   # ratón/teclado simulados contra la interfaz MOCK=1 (necesita Chromium)
```

## Iniciar con la sesión

Ajustes → *Iniciar al iniciar sesión* crea `~/.config/autostart/chroma-desk.desktop` (ejecuta `chroma-desk --hidden`). En Hyprland con uwsm, `systemd-xdg-autostart-generator` ejecuta esas entradas; si la tuya no, añade `chroma-desk --hidden` al autoarranque de tu compositor.

Para mantener vivo el servidor de OpenRGB, activa *Mantener OpenRGB en marcha con un servicio de usuario de systemd* en Ajustes (instala `chroma-desk-openrgb.service`, `openrgb --server --startminimized`). Cierra antes cualquier OpenRGB lanzado a mano.

## Solución de problemas

- **«No se puede contactar con OpenRGB»**: chroma-desk lanza `openrgb --server --startminimized` si nada escucha en el puerto 6742. Comprueba que `openrgb` está instalado y en el `PATH`, o ejecútalo tú con `--server`.
- **Sin dispositivos / faltan RAM o placa**: carga el módulo `i2c-dev` (`echo i2c-dev | sudo tee /etc/modules-load.d/i2c.conf`), instala las reglas udev de OpenRGB (`60-openrgb.rules`), vuelve a iniciar sesión y pulsa *Re-detectar*.
- **Dos aplicaciones peleando por las luces**: la ventana de OpenRGB y chroma-desk escriben en los dispositivos; gana la última que escribe. chroma-desk avisa si hay una ventana de OpenRGB abierta. Inicia OpenRGB con `--startminimized`.
- **Faltan LEDs de ventiladores tras reiniciar**: OpenRGB olvida los tamaños de zona. chroma-desk reaplica los que fijaste en *Avanzado*; asegúrate de que arranca con la sesión o de tener un perfil predeterminado.
- **Sin icono de bandeja**: necesitas un host StatusNotifier (módulo `tray` de Waybar, KDE, extensión de GNOME). Sin él, cerrar la ventana la minimiza.

La configuración está en `~/.config/chroma-desk/` (`config.json`, `profiles/*.json`, `animations/*.json`).

## Licencia

GPL-2.0-or-later. Incluye una copia parcheada del crate `openrgb2` (GPL-2.0) en `src-tauri/vendor/`.
