#!/bin/bash
if [[ -n ${ZSH_VERSION-} ]]; then
	unset ${(M)${(k)parameters[@]}:#__gitcomp_builtin_*} 2>/dev/null
else
	unset $(compgen -v __gitcomp_builtin_)
fi
WD_CONFIG=$1
warp_points=( "${(f)mapfile[$WD_CONFIG]//$HOME/~}" )
while read -r line; do
	arr=(${(s,:,)line})
	echo "${arr[@]}"
done < "$WD_CONFIG"
echo "${(%):-%x}"
