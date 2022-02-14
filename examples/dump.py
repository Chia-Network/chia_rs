import io

from chia_rs import Fullblock


def main():
    blob = open("block-1519806.bin", "rb").read()
    fb = Fullblock.from_bytes(blob)
    print(fb)
    assert bytes(fb) == blob
    prog = fb.generator()
    print()
    print("%s..." % prog.hex()[:80])


if __name__ == "__main__":
    main()
