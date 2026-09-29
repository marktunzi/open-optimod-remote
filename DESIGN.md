# Interface reference

The approved visual target is the supplied full-interface reference at 2264×2108 (not included in the repository), supplemented by the user's meter-edge and header close-ups. The main window is a physical-looking 5700i control surface, while Connections uses a native macOS window, Presets is a workspace of the interface (the native Presets window is only a fallback) and the native System Settings window serves the 5700i.

## Main instrument

- Use SF Compact Medium for controls and the supplied Liquid Crystal Display font only inside the blue status display.
- The graphite chassis background continues behind the header, meters and processing tabs. Its fine noise texture belongs to the chassis and control-well surfaces only. Never draw noise over the meter canvas, meter bars or wells.
- Use the supplied 5700 and Orban assets. The LCD uses the approved dark blue radial/linear material and pale blue characters.
- The FM↔HD, Connected, Setup and Presets controls use 56×22 rounded capsule faces with a slate gradient, fine material texture, restrained inner highlight, lower shade and outer drop shadow. Green LEDs use the approved radial glow.
- Keep all FM and HD meter groups visible at once. Meter wells and lanes have fixed reference widths, live interpolation and individual peak markers.
- The meter group has a clean dark gradient, rounded corners, a subtle half-pixel light inner outline around the shell and a soft white lower inner highlight. It has no dark lower shadow. Group dividers are a dark groove plus a thin lighter adjacent line.
- Processing tabs sit on the same graphite chassis. They are 32 px high with rounded top corners. The active tab uses the approved blue gradient; the row ends in the recessed dark divider plus light highlight from the supplied artwork.
- Processing controls use 151×8 tracks, 28×18 textured metal thumbs and 70×26 value fields. Single-group pages use the approved three-column layout and do not repeat a group title above the well.

## Behavior

The meter strip remains visible above processing. The local FM/HD editing selector appears only while processing paths are decoupled. FM→HD coupling is a global device control with confirmation; physical output routing stays in System Settings.

Controls retain keyboard focus, value fields support Tab and arrow-key editing, and confirmed values remain cyan whenever they differ from the loaded preset baseline. Routine writes do not show banners between meters and controls. Errors go to the native Error Log.

The PC Remote socket has one owner and continues its 50 ms heartbeat and meter polling while terminal snapshots, parameter readback and preset Recall are in progress. A write is never replayed automatically.
