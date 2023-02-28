CURRENT_DIR = $(shell pwd)
.PHONY: all build

all: build

install:
	@if [ ! -f "/usr/bin/apicad" ]; \
        then \
        	ln -s $(CURRENT_DIR)/apicad /usr/bin; \
	fi 
	@if [ ! -f "/usr/bin/a2bc" ]; \
        then \
        	ln -s $(CURRENT_DIR)/bin/a2bc /usr/bin; \
	fi

build: 
	@if [ ! -f "$(CURRENT_DIR)/bin/apicad" ]; \
	then \
		ln -s $(CURRENT_DIR)/apicad $(CURRENT_DIR)/bin; \
	fi
	cd src/analyzer ; cargo build --release

clean:
	make clean -C src/analyzer
	rm /usr/bin/apicad /usr/bin/a2bc

remove-bin:
	rm /usr/bin/apicad /usr/bin/a2bc
