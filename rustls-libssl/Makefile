CARGO ?= cargo
CARGOFLAGS += --locked

CFLAGS := -Werror -Wall -Wextra -Wpedantic $(shell pkg-config --cflags openssl)
CPPFLAGS := -U_FORTIFY_SOURCE -D_FORTIFY_SOURCE=3
RUSTFLAGS += -C target-cpu=native
PROFILE := debug

ifeq ($(PROFILE), debug)
	CFLAGS += -g -fsanitize=address -fsanitize=undefined
	LDFLAGS += -fsanitize=address -fsanitize=undefined
endif

ifeq ($(PROFILE), release)
	CFLAGS += -O3 -fstack-clash-protection
	LDFLAGS += -Wl,-z,now -Wl,--as-needed
	CARGOFLAGS += --release
endif

ifneq (,$(TARGET))
	PROFILE := $(TARGET)/$(PROFILE)
	CARGOFLAGS += --target $(TARGET)
endif

all: target/ciphers target/client target/config target/constants target/server target/$(PROFILE)/libssl.so.3

test: all
	RUSTFLAGS="$(RUSTFLAGS)" ${CARGO} test $(CARGOFLAGS)

integration: all
	RUSTFLAGS="$(RUSTFLAGS)" ${CARGO} test $(CARGOFLAGS) -- --ignored

target:
	mkdir -p $@

target/$(PROFILE)/libssl.so.3: target/$(PROFILE)/libssl.so
	cp -v $^ $@

target/$(PROFILE)/libssl.so: *.rs src/*.rs Cargo.toml
	RUSTFLAGS="$(RUSTFLAGS)" ${CARGO} build $(CARGOFLAGS)

target/%.o: tests/%.c | target
	$(CC) -o $@ -c $< $(CFLAGS) $(CPPFLAGS)

target/ciphers: target/ciphers.o
	$(CC) -o $@ $^ $(LDFLAGS) $(shell pkg-config --libs openssl)

target/client: target/client.o
	$(CC) -o $@ $^ $(LDFLAGS) $(shell pkg-config --libs openssl)

target/config: target/config.o
	$(CC) -o $@ $^ $(LDFLAGS) $(shell pkg-config --libs openssl)

target/constants: target/constants.o
	$(CC) -o $@ $^ $(LDFLAGS) $(shell pkg-config --libs openssl)

target/server: target/server.o
	$(CC) -o $@ $^ $(LDFLAGS) $(shell pkg-config --libs openssl)

clean:
	rm -rf target

format:
	find src tests \
		-name '*.[c|h]' | \
		xargs clang-format -i

format-check:
	find src tests \
		-name '*.[c|h]' | \
		xargs clang-format --dry-run -Werror -i

package: package-deb

package-deb: dist/nfpm.yaml target/release/libssl.so.3 target/VERSION
	mkdir --parents target/dist
	env VERSION=$(shell $(CARGO) get package.version) \
	    VERSION_PRERELEASE=$(shell $(CARGO) get package.version --pre) \
	    nfpm package --config $< --target target/dist --packager deb

target/VERSION: ALWAYS
	echo "This is rustls-libssl `git describe --always`\nDate: `date`\nIncorporating:" > $@
	$(CARGO) tree >> $@

test-package: test-package-deb-22 test-package-deb-24

test-package-deb-22: package-deb
	mkdir --parents target/dist/test/ubuntu-22
	cp target/dist/rustls-libssl_*.deb \
	    dist/test/deb-ubuntu-22/* \
	    test-ca/rsa/server.key \
	    test-ca/rsa/server.cert \
	    test-ca/rsa/ca.cert \
	    target/dist/test/ubuntu-22
	docker build --tag $@ target/dist/test/ubuntu-22
	cd target/dist/test/ubuntu-22 && ./run-test.sh $@

test-package-deb-24: package-deb
	mkdir --parents target/dist/test/ubuntu-24
	cp target/dist/rustls-libssl_*.deb \
	    dist/test/deb-ubuntu-22/* \
	    test-ca/rsa/server.key \
	    test-ca/rsa/server.cert \
	    test-ca/rsa/ca.cert \
	    target/dist/test/ubuntu-24
	# copy deb-ubuntu-24 files on top of -22 ones
	cp dist/test/deb-ubuntu-24/* \
	    target/dist/test/ubuntu-24
	docker build --tag $@ target/dist/test/ubuntu-24
	cd target/dist/test/ubuntu-24 && ./run-test.sh $@

ALWAYS: ;

.PHONY: all clean test integration format format-check package package-deb ALWAYS test-package test-package-deb
