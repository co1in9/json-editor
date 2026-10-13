# JSON Editor icon

`json-editor.svg` is the editable vector source. The blue (`#2563EB`) rounded
tile contains white (`#FFFFFF`) JSON braces and an editing cursor. The artwork
has transparent outer margins for desktop application icons.

`json-editor.png` is the 1024 px raster source. Windows embeds the multi-size
`json-editor.ico` through `build.rs`; the macOS packaging script installs
`json-editor.icns` and declares it in `Info.plist`.

To regenerate with Inkscape and Python Pillow installed, run from the repository
root:

```sh
inkscape assets/icon/json-editor.svg --export-type=png --export-filename=assets/icon/json-editor.png --export-width=1024 --export-height=1024
python scripts/build-icons.py
```

The rounded tile is desktop artwork inspired by iOS icon simplicity. A future
iOS asset catalog should use an opaque, full-bleed square source and let iOS
apply its own corner mask.
