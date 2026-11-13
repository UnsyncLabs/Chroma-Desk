# chroma-desk

English · [Español](#español)

A modern desktop UI for [OpenRGB](https://openrgb.org) (Tauri 2 + Svelte 5 + Rust). It talks to the OpenRGB SDK server, so every device OpenRGB supports works.

## Features

- **Physical keyboard mock-up**: real key widths (ANSI/ISO, TKL/60 %/full), extras row (media keys, dial, logo), matrix fallback.
- **Paint any device LED by LED**: keyboards, matrices, strips, fan rings (pick a fan preset), single LEDs, multi-zone boards. Brush, rectangle selection, eyedropper, fill, eraser, quick groups (WASD, arrows, F-keys…), undo/redo (Ctrl+Z / Ctrl+Shift+Z).
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

If chroma-desk is already running, a second launch focuses it (or applies the profile there).

## Start on login

Settings → *Start on login* creates `~/.config/autostart/chroma-desk.desktop` (runs `chroma-desk --hidden`). On Hyprland with uwsm, `systemd-xdg-autostart-generator` runs these entries; if yours does not, add `chroma-desk --hidden` to your compositor's autostart.

To keep the OpenRGB server alive, enable *Keep OpenRGB running with a systemd user service* in Settings (installs `chroma-desk-openrgb.service`, `openrgb --server --startminimized`). Close any OpenRGB you started by hand first.

## Troubleshooting

- **“Cannot reach OpenRGB”**: chroma-desk starts `openrgb --server --startminimized` if nothing listens on port 6742. Check that `openrgb` is installed and in `PATH`, or run it yourself with `--server`.
- **No devices / RAM or motherboard missing**: load the `i2c-dev` module (`echo i2c-dev | sudo tee /etc/modules-load.d/i2c.conf`) and install OpenRGB's udev rules (`60-openrgb.rules`), then re-login and press *Rescan*.
- **Two apps fighting over the lights**: the OpenRGB window and chroma-desk both write to devices; the last writer wins. chroma-desk warns when an OpenRGB window is open. Start OpenRGB with `--startminimized`.
- **Fan LEDs missing after reboot**: OpenRGB forgets zone sizes. chroma-desk re-applies the sizes you set in *Advanced*; make sure it starts on login or the default profile is set.
- **No tray icon**: you need a StatusNotifier host (Waybar `tray` module, KDE, GNOME extension). Without one, closing the window minimises it.

Config lives in `~/.config/chroma-desk/` (`config.json`, `profiles/*.json`).

## License

GPL-2.0-or-later. Includes a patched copy of the `openrgb2` crate (GPL-2.0) in `src-tauri/vendor/`.

---

# Español

Una interfaz de escritorio moderna para [OpenRGB](https://openrgb.org) (Tauri 2 + Svelte 5 + Rust). Se comunica con el servidor SDK de OpenRGB, así que funciona con todos los dispositivos que OpenRGB soporte.

## Funciones

- **Maqueta física del teclado**: anchos reales de tecla (ANSI/ISO, TKL/60 %/completo), fila de extras (multimedia, dial, logo) y vista de matriz de respaldo.
- **Pintar cualquier dispositivo LED a LED**: teclados, matrices, tiras, anillos de ventilador (con preset), LEDs únicos y placas multizona. Pincel, selección por rectángulo, cuentagotas, relleno, borrador, grupos rápidos (WASD, flechas, F1–F12…) y deshacer/rehacer (Ctrl+Z / Ctrl+Shift+Z).
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

Si chroma-desk ya está en marcha, un segundo lanzamiento lo enfoca (o aplica el perfil ahí).

## Iniciar con la sesión

Ajustes → *Iniciar al iniciar sesión* crea `~/.config/autostart/chroma-desk.desktop` (ejecuta `chroma-desk --hidden`). En Hyprland con uwsm, `systemd-xdg-autostart-generator` ejecuta esas entradas; si la tuya no, añade `chroma-desk --hidden` al autoarranque de tu compositor.

Para mantener vivo el servidor de OpenRGB, activa *Mantener OpenRGB en marcha con un servicio de usuario de systemd* en Ajustes (instala `chroma-desk-openrgb.service`, `openrgb --server --startminimized`). Cierra antes cualquier OpenRGB lanzado a mano.

## Solución de problemas

- **«No se puede contactar con OpenRGB»**: chroma-desk lanza `openrgb --server --startminimized` si nada escucha en el puerto 6742. Comprueba que `openrgb` está instalado y en el `PATH`, o ejecútalo tú con `--server`.
- **Sin dispositivos / faltan RAM o placa**: carga el módulo `i2c-dev` (`echo i2c-dev | sudo tee /etc/modules-load.d/i2c.conf`), instala las reglas udev de OpenRGB (`60-openrgb.rules`), vuelve a iniciar sesión y pulsa *Re-detectar*.
- **Dos aplicaciones peleando por las luces**: la ventana de OpenRGB y chroma-desk escriben en los dispositivos; gana la última que escribe. chroma-desk avisa si hay una ventana de OpenRGB abierta. Inicia OpenRGB con `--startminimized`.
- **Faltan LEDs de ventiladores tras reiniciar**: OpenRGB olvida los tamaños de zona. chroma-desk reaplica los que fijaste en *Avanzado*; asegúrate de que arranca con la sesión o de tener un perfil predeterminado.
- **Sin icono de bandeja**: necesitas un host StatusNotifier (módulo `tray` de Waybar, KDE, extensión de GNOME). Sin él, cerrar la ventana la minimiza.

La configuración está en `~/.config/chroma-desk/` (`config.json`, `profiles/*.json`).

## Licencia

GPL-2.0-or-later. Incluye una copia parcheada del crate `openrgb2` (GPL-2.0) en `src-tauri/vendor/`.
