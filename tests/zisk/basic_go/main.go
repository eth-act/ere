//go:build tamago && riscv64

package main

import (
	"encoding/binary"
	"unsafe"

	"github.com/eth-act/skunkworks-tama/tamaboards/zkvm"
	"github.com/eth-act/skunkworks-tama/tamaboards/zkvm/zisk_runtime"
	"github.com/fxamacker/cbor/v2"
)

// According to crates/test-utils/src/program/basic.rs
type BasicProgramInput struct {
	ShouldPanic bool   `cbor:"should_panic"`
	A           uint8  `cbor:"a"`
	B           uint16 `cbor:"b"`
	C           uint32 `cbor:"c"`
	D           uint64 `cbor:"d"`
	E           []byte `cbor:"e"`
}

// According to crates/test-utils/src/program/basic.rs
type BasicProgramOutput struct {
	E []byte `cbor:"e"`
	D uint64 `cbor:"d"`
	C uint32 `cbor:"c"`
	B uint16 `cbor:"b"`
	A uint8  `cbor:"a"`
}

func readWholeInput() []byte {
	lengthBytes := make([]byte, 8)
	src := unsafe.Pointer(uintptr(zkvm.INPUT_ADDR + 8))
	for i := 0; i < 8; i++ {
		lengthBytes[i] = *(*byte)(unsafe.Add(src, i))
	}
	length := binary.LittleEndian.Uint64(lengthBytes)

	inputBytes := zisk_runtime.UnsafeReadBytes(int(length))
	return inputBytes
}

func unmarshalInput(inputBytes []byte) BasicProgramInput {
	var input BasicProgramInput
	if err := cbor.Unmarshal(inputBytes[4:], &input); err != nil {
		panic("failed to deserialize input")
	}
	return input
}

func compute(input BasicProgramInput) BasicProgramOutput {
	if input.ShouldPanic {
		panic("invalid data")
	}

	output := BasicProgramOutput{
		A: input.A + 1,
		B: input.B + 1,
		C: input.C + 1,
		D: input.D + 1,
		E: make([]byte, len(input.E)),
	}
	for i, b := range input.E {
		output.E[i] = b + 1
	}

	return output
}

func marshalOutput(output BasicProgramOutput) []byte {
	outputBytes, err := cbor.Marshal(output)
	if err != nil {
		panic("failed to serialize output")
	}
	return outputBytes
}

func writeWholeOutput(outputBytes []byte) {
	zisk_runtime.CommitBytes(outputBytes)
}

func main() {
	inputBytes := readWholeInput()
	input := unmarshalInput(inputBytes)
	output := compute(input)
	outputBytes := marshalOutput(output)
	writeWholeOutput(outputBytes)
}
