#!/bin/bash
f() { one; }; f; f() { two; }; f
