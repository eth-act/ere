//go:build tamago && riscv64

package main

import (
	"fmt"
	_ "github.com/eth-act/skunkworks-tama/tamaboards/zkvm"
)

func main() {
	x := 10
	y := 11
	fmt.Println("Hello World", x + y)
}
