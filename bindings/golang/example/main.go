// Command example verifies a zkVM proof through the ere Go binding. It reads the
// encoded program verifying key, the encoded proof, and the expected public
// values from files, then prints the verified public values.
package main

import (
	"bytes"
	"flag"
	"fmt"
	"os"

	ereverifier "github.com/eth-act/ere/bindings/golang"
)

func main() {
	kindName := flag.String("kind", "airbender", "zkvm kind, one of airbender, openvm, risc0, sp1, zisk")
	encodedProgramVKPath := flag.String("vk", "", "path to the encoded program verifying key")
	encodedProofPath := flag.String("proof", "", "path to the encoded proof")
	publicValuesPath := flag.String("pub", "", "path to the expected public values")
	flag.Parse()

	kind, err := parseKind(*kindName)
	if err != nil {
		fail(err)
	}

	encodedProgramVK := mustRead(*encodedProgramVKPath)
	encodedProof := mustRead(*encodedProofPath)
	expectedPublicValues := mustRead(*publicValuesPath)

	verifier, err := ereverifier.New(kind, encodedProgramVK)
	if err != nil {
		fail(err)
	}
	defer verifier.Close()

	publicValues := make([]byte, len(expectedPublicValues))
	if err := verifier.Verify(encodedProof, publicValues); err != nil {
		fail(err)
	}
	if !bytes.Equal(publicValues, expectedPublicValues) {
		fail(fmt.Errorf("public values mismatch, got %x want %x", publicValues, expectedPublicValues))
	}

	fmt.Printf("verified, public values %x\n", publicValues)
}

func parseKind(name string) (ereverifier.ZkVMKind, error) {
	switch name {
	case "airbender":
		return ereverifier.Airbender, nil
	case "openvm":
		return ereverifier.OpenVM, nil
	case "risc0":
		return ereverifier.Risc0, nil
	case "sp1":
		return ereverifier.SP1, nil
	case "zisk":
		return ereverifier.Zisk, nil
	default:
		return 0, fmt.Errorf("unknown kind %q", name)
	}
}

func mustRead(path string) []byte {
	if path == "" {
		fail(fmt.Errorf("a required file path was empty"))
	}
	data, err := os.ReadFile(path)
	if err != nil {
		fail(err)
	}
	return data
}

func fail(err error) {
	fmt.Fprintln(os.Stderr, "error:", err)
	os.Exit(1)
}
