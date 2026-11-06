#!/bin/bash
exec {origout}>&1- >tmp.log 2>&1; bar
