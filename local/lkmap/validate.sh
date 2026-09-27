#!/bin/sh
# Checks transfer files for the Leitungskataster (SIA405 LKMap, Zuständigkeitsperimeter) with
# ilivalidator against the models of the SIA and the canton, like the canton's Checkservice:
#
#   local/lkmap/validate.sh <file.xtf> ...
#
# Downloads ilivalidator once into ${XDG_CACHE_HOME:-~/.cache}/cable-editor/ (the models land
# in its ilicache there); Java comes from the PATH or from nix. The licensed SIA models never
# enter the repository (see docs/leitungskataster.md). Exits non-zero if a file has errors.
set -e
VERSION=1.15.0
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/cable-editor"
JAR="$CACHE/ilivalidator-$VERSION/ilivalidator-$VERSION.jar"
MODELDIR="https://405.sia.ch/models;https://models.geo.zh.ch;https://models.interlis.ch"

if [ ! -f "$JAR" ]; then
  mkdir -p "$CACHE"
  echo "ilivalidator $VERSION laden ..." >&2
  curl -fsSL -o "$CACHE/ilivalidator.zip" "https://downloads.interlis.ch/ilivalidator/ilivalidator-$VERSION.zip"
  (cd "$CACHE" && unzip -qo ilivalidator.zip -d "ilivalidator-$VERSION" && rm ilivalidator.zip)
  # Some releases unpack into a folder of their own
  [ -f "$JAR" ] || JAR=$(find "$CACHE/ilivalidator-$VERSION" -name "ilivalidator-$VERSION.jar" | head -1)
fi
[ -f "$JAR" ] || JAR=$(find "$CACHE/ilivalidator-$VERSION" -name "ilivalidator-$VERSION.jar" | head -1)

if command -v java >/dev/null 2>&1; then
  set -- java "-Duser.home=$CACHE" -jar "$JAR" --modeldir "$MODELDIR" "$@"
else
  set -- nix shell nixpkgs#jdk21_headless -c java "-Duser.home=$CACHE" -jar "$JAR" --modeldir "$MODELDIR" "$@"
fi
exec "$@"
