# Forge

Forge is a terminal CAD tool for preparing 3D-printing models. It runs in the [Kitty terminal](https://sw.kovidgoyal.net/kitty/).

## Download

The current release is for 64-bit Linux.

```bash
curl -LO https://github.com/basanta-bhandari/Forge/releases/latest/download/forge-linux-x86_64.tar.gz
tar -xzf forge-linux-x86_64.tar.gz
cd forge-0.1.0
```

## Run

Open the included demo:

```bash
./forgetui --demo
```

Or open a model:

```bash
./forgetui model.stl
```

## SSH

Forge works over SSH when you connect from Kitty. Install Forge on the remote machine, then connect and run it there:

```bash
kitten ssh user@server
./forgetui --demo
```
