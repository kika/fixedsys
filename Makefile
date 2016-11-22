COMPILE=ttx -f --recalc-timestamp 
CPP=cpp

all: FSEX302.ttf FSEX302-alt.ttf

FSEX302.ttf: FSEX-default.ttx
	$(COMPILE) -o $@ $<
	cp $@ ~/Library/Fonts
	atsutil databases -remove
	rm $<

FSEX302-alt.ttf: FSEX-alt.ttx
	$(COMPILE) -o $@ $<
	rm $<

FSEX-default.ttx: FSEX.ttx
	$(CPP) $< |grep -v "^#" > $@ 

FSEX-alt.ttx: FSEX.ttx
	$(CPP) -DALT_LESSEQUAL $< |grep -v "^#" > $@
