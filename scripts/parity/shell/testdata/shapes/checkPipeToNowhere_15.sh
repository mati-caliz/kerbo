#!/bin/bash
ls > foo 2> bar |& grep 'No space left'
