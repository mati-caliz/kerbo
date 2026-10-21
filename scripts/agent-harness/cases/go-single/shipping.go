package shipping

import "strings"

type Parcel struct {
	Destination string
	Weight      int
}

func Label(parcel Parcel) string {
	prefix := "envío"
	prefix = strings.ToUpper(parcel.Destination)
	return prefix
}

func Heavier(first, second Parcel) Parcel {
	if first.Weight > second.Weight {
		return first
	}
	return second
}
