# The preview of an effect

The page Effect has a preview card: the particles of the open effect at a frame, with buttons to play, pause, restart and repeat, a slider for the frame, and a view that the user turns with the mouse and zooms with the wheel. The preview follows the edits at once: a change of a colour, a size, a life or a number of particles shows at the next picture.

## What it shows

- When the particles appear: the start, the interval, the span and the number of particles at each emission of each layer.
- Where: the shape of the emitter (a point, a box, a rectangle, a disc, a cylinder or a sphere, with its size, its empty part and its arc).
- For how long: the life of each particle, between its shortest and its longest value.
- How they look: the size, the colour and the transparency during the life, and the picture of the layer. A picture that holds several frames side by side shows its first frame.
- How they move: a particle goes in a straight line with the speed and the direction that its layer gives at the start (the beam of fire shoots its particles along its axis), and it keeps a part of its speed at each frame (an air resistance). The pictures turn: a start angle (random when the layer says so) and a speed of turn. The emitter is at the place that its object gives.

The particles of a frame are computed from the frame alone, so the user can go to any frame. The same frame always gives the same particles. The drawing is made on the processor (`module/effect/src/preview.rs`): each particle is a flat picture facing the viewer, added to a dark background like the light effects of the game.

## What it does not show

- The other forces: gravity, wind, the speed away from the centre of the emitter and the other rules of movement of the particles are not read. Many rules of the files are known only by their numbers (see below), so a layer that moves with such a rule stays where its first speed puts it.
- The animation of a picture (the frames of a strip), the mix mode of the material, the order of drawing and the layers of the models.
- Which picture a layer uses is guessed: the picture whose name is in the name of the set or of the emitter, else the picture of the same place.

## How the movement was read

The rules of a layer are objects of the file: a type, the number 256, a target, then the values. The meaning of the types of the target 12 was found from the values of the 2040 effects of the game, not from a description of the format:

| Rule | Values | Meaning |
| --- | --- | --- |
| Initializer, type `0x80000000`, target 12 | a speed, then a direction of 3 numbers | The speed at the start in units for each frame. The beam of fire has 3 and the direction (0, 0, -1); the smoke of an ice breath has 0.5 and (0, 1, 0), so it rises. |
| Updater, type `0x80000000`, target 12 | one number between 0.8 and 0.97 | The part of the speed that is kept at each frame. |
| Initializer, type `0x100000`, target 2 | a first angle on 3 axes, then a number for each axis | The angle of the picture at the start. |
| Updater, type `0x800002`, target 2 | 3 numbers | The turn at each frame. |

The other rules of the target 12 and of the target 0 have values that look like speeds, forces and directions, but their meaning is not sure, so they are not used. The preview shows what the page edits and the main movement; it is not a copy of the game.
