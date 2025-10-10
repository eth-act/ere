#!/bin/bash

set -e -o pipefail

df -h /
sudo rm -rf /usr/share/dotnet
sudo rm -rf /usr/local/lib/android
sudo rm -rf /opt/hostedtoolcache
df -h /
