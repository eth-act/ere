//go:build cgo

// Package ereverifier wraps ere-verifier-c through cgo.
//
// A *Verifier may be used from multiple goroutines for concurrent Verify and
// Kind calls. Close consumes the handle and must not overlap with other calls
// on the same Verifier.
package ereverifier

/*
#cgo linux  LDFLAGS: -lere_verifier_c -lm -lpthread -ldl
#cgo darwin LDFLAGS: -lere_verifier_c

#include "ere_verifier.h"
*/
import "C"

import (
	"errors"
	"fmt"
	"runtime"
	"unsafe"
)

// ZkVMKind mirrors the Rust `ere_verifier::zkVMKind` enum. Values are part
// of the public ABI and must match the declaration order on the Rust side.
type ZkVMKind uint32

const (
	Airbender ZkVMKind = 0
	OpenVM    ZkVMKind = 1
	Risc0     ZkVMKind = 2
	SP1       ZkVMKind = 3
	Zisk      ZkVMKind = 4
)

// String implements [fmt.Stringer].
func (k ZkVMKind) String() string {
	switch k {
	case Airbender:
		return "airbender"
	case OpenVM:
		return "openvm"
	case Risc0:
		return "risc0"
	case SP1:
		return "sp1"
	case Zisk:
		return "zisk"
	default:
		return fmt.Sprintf("unknown(%d)", uint32(k))
	}
}

var (
	// ErrNullPtr indicates a required pointer argument was null.
	ErrNullPtr = errors.New("ere: null pointer")
	// ErrBadKind indicates the zkvm_kind value is not one of the documented variants.
	ErrBadKind = errors.New("ere: unsupported zkvm_kind")
	// ErrDecodeProgramVK indicates the program verifying key bytes failed to decode.
	ErrDecodeProgramVK = errors.New("ere: failed to decode program verifying key")
	// ErrDecodeProof indicates the proof bytes failed to decode.
	ErrDecodeProof = errors.New("ere: failed to decode proof")
	// ErrVerify indicates the proof was well-formed but failed cryptographic verification.
	ErrVerify = errors.New("ere: proof failed verification")
	// ErrPublicValuesBufferTooSmall indicates the proof verified but the
	// public_values buffer is shorter than the proven public values.
	ErrPublicValuesBufferTooSmall = errors.New("ere: public values buffer too small")
	// ErrPublicValuesBufferTooLarge indicates the proof verified but the
	// public_values buffer is longer than the proven public values.
	ErrPublicValuesBufferTooLarge = errors.New("ere: public values buffer too large")
	// ErrInternal indicates an unexpected internal condition that reflects a bug
	// in the binding or the verifier library rather than an invalid argument.
	ErrInternal = errors.New("ere: internal error")
)

func statusToError(status C.int32_t) error {
	switch status {
	case C.ERE_OK:
		return nil
	case C.ERE_ERR_NULL_PTR:
		return ErrNullPtr
	case C.ERE_ERR_BAD_KIND:
		return ErrBadKind
	case C.ERE_ERR_DECODE_PROGRAM_VK:
		return ErrDecodeProgramVK
	case C.ERE_ERR_DECODE_PROOF:
		return ErrDecodeProof
	case C.ERE_ERR_VERIFY:
		return ErrVerify
	case C.ERE_ERR_PUBLIC_VALUES_BUFFER_TOO_SMALL:
		return ErrPublicValuesBufferTooSmall
	case C.ERE_ERR_PUBLIC_VALUES_BUFFER_TOO_LARGE:
		return ErrPublicValuesBufferTooLarge
	case C.ERE_ERR_INTERNAL:
		return ErrInternal
	default:
		return fmt.Errorf("ere: unknown error code %d", int32(status))
	}
}

// bytePtr returns &buffer[0] as *C.uint8_t, or nil for the empty slice. Passing
// the nil pointer to C matches the (NULL, 0) convention the Rust side accepts.
func bytePtr(buffer []byte) *C.uint8_t {
	if len(buffer) == 0 {
		return nil
	}
	return (*C.uint8_t)(unsafe.Pointer(&buffer[0]))
}

// Verifier is a handle to a zkVM verifier bound to a specific program
// verifying key. A Verifier is constructed with [New] and released with
// [Verifier.Close].
type Verifier struct {
	handle *C.EreVerifier
}

// New constructs a verifier bound to encodedProgramVK. The returned handle
// is released either explicitly via [Verifier.Close] or by the runtime
// finalizer.
func New(kind ZkVMKind, encodedProgramVK []byte) (*Verifier, error) {
	var handle *C.EreVerifier
	status := C.ere_verifier_new(
		C.uint32_t(kind),
		bytePtr(encodedProgramVK), C.uintptr_t(len(encodedProgramVK)),
		&handle,
	)
	runtime.KeepAlive(encodedProgramVK)
	if err := statusToError(status); err != nil {
		return nil, err
	}
	v := &Verifier{handle: handle}
	runtime.SetFinalizer(v, (*Verifier).Close)
	return v, nil
}

// Close releases the underlying verifier. It is safe to call more than once
// and on a nil receiver.
func (v *Verifier) Close() {
	if v == nil || v.handle == nil {
		return
	}
	C.ere_verifier_free(v.handle)
	v.handle = nil
	runtime.SetFinalizer(v, nil)
}

// Kind returns the zkVM the verifier was constructed for. It returns
// [ErrNullPtr] for a nil receiver or a closed verifier.
func (v *Verifier) Kind() (ZkVMKind, error) {
	if v == nil || v.handle == nil {
		return 0, ErrNullPtr
	}
	var output C.uint32_t
	status := C.ere_verifier_zkvm_kind(v.handle, &output)
	if err := statusToError(status); err != nil {
		return 0, err
	}
	return ZkVMKind(output), nil
}

// Verify checks encodedProof against the verifier's program verifying key and
// copies the proven public values into publicValues, which the caller sizes.
//
// On success publicValues is filled with len(publicValues) bytes. The proven
// public values may be longer than publicValues only when the bytes past it
// are all zero, which accommodates proof systems that pad public values to a
// fixed length. A buffer longer than the proven public values returns
// [ErrPublicValuesBufferTooLarge]. A shorter buffer returns
// [ErrPublicValuesBufferTooSmall] unless the dropped trailing bytes are all
// zero, in which case the leading bytes are copied and the call succeeds.
func (v *Verifier) Verify(encodedProof []byte, publicValues []byte) error {
	if v == nil || v.handle == nil {
		return ErrNullPtr
	}
	status := C.ere_verifier_verify(
		v.handle,
		bytePtr(encodedProof), C.uintptr_t(len(encodedProof)),
		bytePtr(publicValues), C.uintptr_t(len(publicValues)),
	)
	runtime.KeepAlive(encodedProof)
	runtime.KeepAlive(publicValues)
	return statusToError(status)
}
