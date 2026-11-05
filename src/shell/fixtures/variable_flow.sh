#!/bin/bash
echo "$never_assigned" "${never_guarded:-x}" "${never_checked:?}" "${#never_length}"
echo "${guard_outer:+$guard_inner}" "${array_name[index_name]}" "${offset_name:start_name}"
assigned_later=1
echo "$assigned_later" "$ASSIGNED_UPPER" "$ASSIGNED_UPPE"
ASSIGNED_UPPER=1
[ -n "$checked_n" ] && echo "$checked_n"
[ -z "$checked_z" ]
[ "$checked_nullary" ]
[[ -v checked_set ]] && echo "$checked_set"
echo "${defaulted:=1}" "$defaulted"
export exported_name
echo "$exported_name"
declare declared_name
echo "$declared_name"
read -r read_name
echo "$read_name"
exec {descriptor}>/dev/null
echo "$descriptor"
exec {closed}>&-
for looped in a; do echo "$looped"; done
echo "$looped"
echo "$HOME" "$UID" "$USERNAME" "$RANDOM" "$MISSING_UPPER"
[[ a = $pattern_read* ]]
echo "$@" "$*" "$1" "$?"
(( math_read + 1 ))
(( math_written = 1 ))
echo "$math_written"
declare -A map_name
map_name[key_name]=1
other_list[index_read]=1
echo "$other_list"
( in_group=1 )
echo "$in_group"
echo x | { in_pipe=1; }
echo "$in_pipe"
in_last=$(in_sub=1; echo "$in_sub")
echo "$in_last" "$in_sub"
sleep 1 & in_background=1
echo "$in_background"
{ in_list=1; } &
echo "$in_list"
( first_write=1; first_write=2 )
echo "$first_write"
( revived=1 )
revived=2
echo "$revived"
( declared_only=1 )
declare declared_only
echo "$declared_only"
cat f | while read -r piped; do echo "$piped"; done
echo "$piped"
( IFS=: )
echo "$IFS"
( counter=1 )
(( counter++ ))
echo "$counter"
<(proc_sub=1)
echo "$proc_sub"
