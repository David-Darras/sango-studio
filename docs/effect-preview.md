# The preview of an effect

The page Effect has a preview card: the particles of the open effect at a frame, with buttons to play, pause, restart and repeat, a slider for the frame, and a view that the user turns with the mouse and zooms with the wheel. The preview follows the edits at once: a change of a colour, a size, a life or a number of particles shows at the next picture.

## The rules of the game

An effect file holds, for each layer, an emitter, a shape and two lists of rules: the *initializers* give a first value to the streams of a new particle, and the *updaters* change a stream at each frame. The preview follows these rules (`module/effect/src/preview.rs`, the types in `effect_file.rs`).

The streams of a particle: 0 the place, 1 the size, 2 the turn, 3 the colour, 4 the transparency, 5 to 7 the picture coordinates, 8 the extended size, 11 the life, 12 the speed. A rule names its stream (its target). The types of the rules are numbers of the file (a bit each); the data of an initializer starts at the byte 12 of the object, the data of an updater at the byte 16.

| Initializer (type) | Data | Effect |
| --- | --- | --- |
| `0x80000000` | power, direction | adds the speed `direction × power` |
| `0x40000000` | power, direction, angle | adds a speed in a cone around the direction |
| `0x20000000` | power | adds a speed away from the centre of the emitter |
| `0x10000000` | power | adds a speed in a random direction |
| `0x08000000` | power | adds a speed away from the vertical axis |
| `0x04000000` | 3 numbers | sets a stream (the colour) |
| `0x02000000` | base, random | base plus or minus a part of it (the transparency) |
| `0x01000000` | max, min | a whole number between the two (the life) |
| `0x00200000` | base (3), random | base, all axes changed by the same random part (the size) |
| `0x00100000` | base (3), random (3) | each axis changed by its own random part (the turn) |
| `0x00080000` | random | multiplies the speed by a number around 1 |

| Updater (type) | Data | Effect at each frame |
| --- | --- | --- |
| `0x80000000` | factor | multiplies the stream by the factor (air resistance on the speed) |
| `0x40000000` | none | moves the particle by its speed |
| `0x20000000` | power, direction | adds `direction × power` to the stream (gravity on the speed) |
| `0x10000000` | power, axis | turns the stream around the axis |
| `0x08000000` | power (3), interval | adds a random number |
| `0x00020000` | power, place, axes | pulls the stream toward the place |
| `0x00800002` | 3 numbers | adds the numbers (the turn of the picture) |
| `0x00200000` | keys | follows 4 keys during the life (the transparency) |
| `0x00100000` | keys | follows 4 keys during the life (the colour, the size) |

The emitter makes particles from the frame after its start: at each frame the number of the layer (with its random) is added to a count, the whole part is made, and the next emission is `interval` frames later; the first emission makes at least one particle. The place of a new particle comes from the shape of the emitter: the size of a shape is its radius (half of its width for a box); a disc or a rectangle is flat (the axes X and Z), a sphere is denser near its surface. These rules were read in the source of the system, not guessed.

A particle is computed from its birth to the frame by going through the frames one after the other, so the user can go to any frame and the same frame always gives the same particles (the random numbers come from a seed made of the layer, the frame of the birth and the number of the particle). The drawing is flat: a picture facing the viewer, added to a dark background.

## The material and the way that a particle is drawn

- The material of a layer (the one that has the name of its set plus `Material`) gives the picture (the texture mapper points at the picture object), the way that the particle is mixed with what is behind it, and the coordinates of the picture. The mix is read from the blend function of the material: a particle whose destination factor is "one" is added (light), the others use their transparency.
- The way that the game draws the particle comes from the shape builder of the set: a picture that faces the screen (`0x80000000`), the same in the space of the world (`0x40000000`), a picture that stays upright and turns around the vertical axis (`0x20000000`, the bolts of a lightning), or a flat picture in the plane of the emitter (`0x10000000`).
- The constant values of a layer (the parameters of its collection) are the first value of each stream; the initializers and the updaters then replace them.
- A rule can follow a curve (an animation): the frame comes from the age of the particle, stretched over its life, repeated, or random. The values between two frames are mixed. The bolts of a lightning use this to show one of four drawings.
- The size of a particle is its size (stream 1) multiplied by its second size (stream 8). The game puts the corners of the picture at plus and minus this size, so it is a half size: the picture is twice as wide and as high.
- A rule of the type `0x00040000` cuts the picture in a grid and shows one cell; the number of the cell can follow a curve of the life. A picture of four bolts side by side is four frames of one bolt, shown one after the other.
- The coordinates of the picture: the translation and the scale of the particle (streams 5 and 6) turn around the middle of the picture, then the material changes them with its matrix. The material says how the matrix is made (its mode): the usual mode gives `scale × (coordinate - translation)`, the mode 2 turns around the middle of the picture (`scale × (coordinate - translation - 0.5) + 0.5`, and the translation of the V axis has the other sign). The V axis of the chip goes up, the one of the picture goes down. The picture is filtered linearly. Outside of the picture the material says what happens: the edge goes on, nothing is drawn, the picture repeats, or it repeats turned over.

## What it does not do

- The animation of the other options of a curve (for example a delay) and the curves of the initializers.
- The child particles, the collisions, the turn that points a picture along its speed, and the matrix of the emitter.
- Which picture a layer uses is read from the material; a layer without material uses the picture whose name is in the name of its set.

The preview shows what the page edits and the movement of the particles. It is not a copy of the game, and it was not compared with the game picture by picture.
