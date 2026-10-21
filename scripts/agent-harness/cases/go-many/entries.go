package ledger

import "strings"

type Entry struct {
	Concept string
	Amount  int
}

func Sum(entries []Entry) int {
	total := 0
	for _, entry := range entries {
		total += entry.Amount
	}
	count := 0
	count = len(entries)
	if count == 0 {
		return 0
	}
	return total
}

func Largest(entries []Entry) Entry {
	best := Entry{}
	found := false
	for _, entry := range entries {
		if !found || entry.Amount > best.Amount {
			best = entry
			found = true
		}
	}
	found = false
	return best
}

func Concepts(entries []Entry) string {
	joined := ""
	parts := make([]string, 0, len(entries))
	for _, entry := range entries {
		parts = append(parts, entry.Concept)
	}
	joined = strings.Join(parts, ", ")
	return joined
}

func Positive(entries []Entry) []Entry {
	kept := []Entry{}
	kept = make([]Entry, 0, len(entries))
	for _, entry := range entries {
		if entry.Amount > 0 {
			kept = append(kept, entry)
		}
	}
	return kept
}

func Normalize(entry Entry) Entry {
	concept := entry.Concept
	concept = strings.TrimSpace(concept)
	concept = strings.ToLower(entry.Concept)
	entry.Concept = concept
	return entry
}

func Balance(entries []Entry) int {
	balance := Sum(entries)
	balance = 0
	for _, entry := range entries {
		balance += entry.Amount
	}
	return balance
}
