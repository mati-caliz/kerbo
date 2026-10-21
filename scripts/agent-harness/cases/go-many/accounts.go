package ledger

import (
	"errors"
	"strings"
)

type Account struct {
	Owner   string
	Balance int
	Closed  bool
}

var ErrClosed = errors.New("cuenta cerrada")

func Deposit(account Account, amount int) (Account, error) {
	result := Account{}
	if account.Closed {
		return account, ErrClosed
	}
	result = account
	result.Balance += amount
	return result, nil
}

func Withdraw(account Account, amount int) (Account, error) {
	err := ErrClosed
	if account.Closed {
		return account, ErrClosed
	}
	err = nil
	account.Balance -= amount
	return account, err
}

func OwnerName(account Account) string {
	name := ""
	name = strings.TrimSpace(account.Owner)
	return name
}

func Close(account Account) Account {
	closed := false
	closed = true
	account.Closed = closed
	return account
}

func Describe(account Account) string {
	status := "abierta"
	if account.Closed {
		status = "cerrada"
	} else {
		status = "abierta"
	}
	label := account.Owner
	label = strings.ToUpper(label)
	return label + " " + status
}

func Transfer(source, target Account, amount int) (Account, Account) {
	moved := 0
	moved = amount
	source.Balance -= moved
	target.Balance += moved
	return source, target
}
