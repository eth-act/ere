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
	SP1       ZkVMKind = 2
	Zisk      ZkVMKind = 3
)

// String implements [fmt.Stringer].
func (k ZkVMKind) String() string {
	switch k {
	case Airbender:
		return "airbender"
	case OpenVM:
		return "openvm"
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

// Verifier is a handle to a verifier bound to a program verifying key. It is
// created with [New] and released with [Verifier.Close].
type Verifier struct {
	handle *C.EreVerifier
}

// New constructs a verifier bound to encodedProgramVK for the given zkVM kind.
// The handle is released with [Verifier.Close] or, as a fallback, by the
// garbage collector finalizer.
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

// Verify checks encodedProof against the verifier's program verifying key and
// returns the proven public values.
//
// The returned slice is owned by Go and managed by the garbage collector. The
// native buffer is copied out and released within this call, so the caller
// sizes nothing and frees nothing. Empty public values yield an empty, non-nil
// slice.
func (v *Verifier) Verify(encodedProof []byte) ([]byte, error) {
	if v == nil || v.handle == nil {
		return nil, ErrNullPtr
	}
	var ptr *C.uint8_t
	var length C.uintptr_t
	status := C.ere_verifier_verify(
		v.handle,
		bytePtr(encodedProof), C.uintptr_t(len(encodedProof)),
		&ptr, &length,
	)
	runtime.KeepAlive(encodedProof)
	runtime.KeepAlive(v)
	if err := statusToError(status); err != nil {
		return nil, err
	}
	if ptr == nil || length == 0 {
		return []byte{}, nil
	}
	publicValues := C.GoBytes(unsafe.Pointer(ptr), C.int(length))
	C.ere_bytes_free(ptr, length)
	return publicValues, nil
}

// Kind returns the zkVM kind the verifier was constructed for. It returns
// [ErrNullPtr] for a nil receiver or a closed verifier.
func (v *Verifier) Kind() (ZkVMKind, error) {
	if v == nil || v.handle == nil {
		return 0, ErrNullPtr
	}
	var output C.uint32_t
	status := C.ere_verifier_zkvm_kind(v.handle, &output)
	runtime.KeepAlive(v)
	if err := statusToError(status); err != nil {
		return 0, err
	}
	return ZkVMKind(output), nil
}

// Close releases the underlying verifier. It is safe to call more than once and
// on a nil receiver.
func (v *Verifier) Close() {
	if v == nil || v.handle == nil {
		return
	}
	C.ere_verifier_free(v.handle)
	v.handle = nil
	runtime.SetFinalizer(v, nil)
}
