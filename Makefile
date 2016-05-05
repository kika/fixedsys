FSEX302.ttf: FSEX.ttx
	ttx -f -o $@ $<
	cp $@ ~/Library/Fonts
	atsutil databases -remove

