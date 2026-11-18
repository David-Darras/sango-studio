# The rules of the interface

The rules that the pages of Sango Studio follow, and what each one means in the code.

| Rule | What the user sees | Where |
| --- | --- | --- |
| The user sees what can be edited | The number boxes, the text boxes and the menus have an outline; a number box has a tip (click to type, drag, arrows). | `theme::apply`, `widget::spin` |
| The user sees the state | "Not saved", "From the SD card" and "From the romfs" next to the Save button; a message tells the result of an action. | each page |
| The user can undo | Undo on the pages, and "Reset to the game" for what was saved. | each page |
| The user may make an error | An action that cannot be undone (reset, remove, replace the team, back to the model of the game) asks a second click. | `widget::confirm_button` |
| Nothing is lost without a word | The window asks before it closes with changes that are not saved; the Trainer page asks before it opens another trainer. | `Module::unsaved`, `App::close_guard`, `Trainer::pending_dialog` |
| A disabled button tells why | Save is off when nothing changed, and a card tells that the SD card is not chosen. | `Trainer::tool_bar` |
| The keyboard works as the mouse | A click gives the focus; the arrows change a number or a choice; Enter closes a list. | `widget::set_active_id`, `trainer/focus.rs` |
| Legal by default | The team builder shows what is legal in Gen 6 first, and warns about the rest. | `trainer/builder.rs` |
