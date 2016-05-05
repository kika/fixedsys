FSEX301.ttf: FSEX301.ttx
	ttx -f $<
	cp $@ ~/Library/Fonts
	atsutil databases -remove

