#!/usr/bin/env bash

step() {
  printf "\n\033[36m>> %s\033[0m\n" "$1"
}

ok() {
  printf "   \033[32m%s\033[0m\n" "$1"
}
