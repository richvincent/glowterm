# glowterm

> Run a shell with an animation behind its text.
> Press `Ctrl + ]` to switch animation; exit the shell to leave.

- Start the login shell over the default animation (Matrix rain):

`glowterm`

- Start with a specific animation:

`glowterm {{[-a|--anim]}} {{matrix|aurora|fire|starfield|plasma|cyberspace|nightride|arcade}}`

- List the available animations:

`glowterm {{[-l|--list]}}`

- Run a command instead of the shell:

`glowterm -- {{command}} {{argument1 argument2 ...}}`

- Dim the animation overall and remove it completely from behind lines of text:

`glowterm {{[-i|--intensity]}} {{0.6}} {{[-t|--text-dim]}} {{0}}`

- Make the animation less busy and draw fewer frames per second, to lighten the load on the terminal:

`glowterm {{[-d|--density]}} {{0.5}} {{[-f|--fps]}} {{10}}`

- Leave the mouse to the terminal, so click-drag selects text normally (the wheel no longer scrolls history):

`glowterm --no-mouse`

- Pass `Ctrl + ]` through to the program instead of using it to switch animation:

`glowterm --no-hotkey`
