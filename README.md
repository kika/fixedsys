# Fixedsys Excelsior font with programming ligatures

## Original copyright
Created by Darien Valentine

[Font website](http://www.fixedsysexcelsior.com)

## It looks like this
<img src="./images/sample.png" />

## Where to get the result without compiling
The compiled TTF binary font is on the Releases page.

## Rationale
I was always jealous for folks using [Fira Code](https://github.com/tonsky/FiraCode), 
[Hasklig](https://github.com/i-tu/Hasklig) or [Monoid](https://github.com/larsenwork/monoid) 
fonts, but my problem is that I have a hard time reading (not even mentioning writing) 
a computer program in anything but 8x16 font. I probably spent too much time with older
computers. So after fighting and losing an uphill battle with Glyph2, Fontlab and Fontforge
I discovered [TTX](https://github.com/behdad/fonttools) and was able to finally stop being jealous. 

This current release has almost everything I use regularly, besides `<$>` and 
`<*>` which I will add soon. I'm also thinking about doing ligatures for 
`{-` and `-}`. Other than that feel free to request in the issues. PRs are of course 
more than welcome. I hope I'm not the only crazy guy on the internet using 8x16
font for consoles and text editing. 

## History
This font is a simulated 8x16 bitmap font from old Windows and DOS. It was 
truly monospaced and really bitmapped and initially contained only Western ASCII
charset. 
Darien simulated the bitmap with TrueType outlines by building the font from
10x10 squares ("pixels") and then joining the squares together. As such, this font
only works as intended in only one size and usually with antialiasing switched
off. The size is 16px or 12pt. 

He also added a lot of foreign characters and made the font Unicode. 

## Tech trivia
The font is distributed in binary TTF format and I decompiled it with 
[TTX](https://github.com/behdad/fonttools), added a few symbols inspired by 
[Fira Code](https://github.com/tonsky/FiraCode) and created necessary ligatures.

To design the symbols I used quad lined paper, pencil and rubber eraser. Like
in good old days, you know. 
<img src="./images/IMG_3506.jpg" />

The supported programming ligatures are listed in the `ligatures.txt` file.

To create a TTF file from TTX XML just run `ttx -f FSEX.ttx` 
(`-f` means overwrite) or use OS X Makefile to also copy to the user Fonts
folder and update font cache.

I only tested in MacVim so far (this is the only editor I use). Comments and bug
reports welcome. MacVim should be quite recent for ligatures to work.

Add the following to your `.vimrc`:

```
set guifont=Fixedsys\ Excelsior:h16
set noanti 
set macligatures
```
## ToDo
<| |> <* *> <$ $>

