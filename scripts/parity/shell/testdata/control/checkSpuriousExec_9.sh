#!/bin/bash
for file in rc.d/*; do exec "$file"; done
