#!/bin/bash
find . -name '*.wav' -o \( -name '*.au' -exec rm {} + \)
