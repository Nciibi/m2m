.PHONY: build image clean

build:
	cd .. && cargo build --release -p m2m-relay

image:
	docker build -f Dockerfile -t m2m-relay ..

clean:
	rm -f m2m-relay
