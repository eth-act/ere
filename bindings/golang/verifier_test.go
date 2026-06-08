//go:build cgo

package ereverifier

import (
	"bytes"
	"errors"
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

type fixture struct {
	name string
	kind ZkVMKind
}

var fixtures = []fixture{
	{"airbender", Airbender},
	{"openvm", OpenVM},
	{"risc0", Risc0},
	{"sp1", SP1},
	{"zisk", Zisk},
}

// workspaceRoot returns the absolute path of the repo root, derived from this
// test file's location at bindings/golang/verifier_test.go.
func workspaceRoot(t *testing.T) string {
	t.Helper()
	_, file, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("runtime.Caller failed")
	}
	return filepath.Join(filepath.Dir(file), "..", "..")
}

func mustRead(t *testing.T, path string) []byte {
	t.Helper()
	b, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read %s: %v", path, err)
	}
	return b
}

func TestVerifier(t *testing.T) {
	root := workspaceRoot(t)
	for _, fx := range fixtures {
		t.Run(fx.name, func(t *testing.T) {
			dir := filepath.Join(root, "crates", "verifier", fx.name, "tests", "fixtures")
			programVK := mustRead(t, filepath.Join(dir, "program_vk.bin"))
			proof := mustRead(t, filepath.Join(dir, "proof.bin"))
			publicValues := mustRead(t, filepath.Join(dir, "public_values.bin"))

			t.Run("verify", func(t *testing.T) {
				v, err := New(fx.kind, programVK)
				if err != nil {
					t.Fatalf("New: %v", err)
				}
				defer v.Close()

				kind, err := v.Kind()
				if err != nil || kind != fx.kind {
					t.Fatalf("Kind() = %v, %v, want %v", kind, err, fx.kind)
				}

				out, err := v.Verify(proof)
				if err != nil {
					t.Fatalf("Verify: %v", err)
				}
				if !bytes.Equal(out, publicValues) {
					t.Fatalf("public values = %x, want %x", out, publicValues)
				}
			})

			t.Run("invalid_program_vk_decode", func(t *testing.T) {
				if _, err := New(fx.kind, programVK[:len(programVK)-1]); !errors.Is(err, ErrDecodeProgramVK) {
					t.Fatalf("truncated vk: got %v, want ErrDecodeProgramVK", err)
				}
				extended := append(append([]byte{}, programVK...), 0xFF)
				if _, err := New(fx.kind, extended); !errors.Is(err, ErrDecodeProgramVK) {
					t.Fatalf("extended vk: got %v, want ErrDecodeProgramVK", err)
				}
			})

			t.Run("invalid_proof_decode", func(t *testing.T) {
				v, err := New(fx.kind, programVK)
				if err != nil {
					t.Fatalf("New: %v", err)
				}
				defer v.Close()

				if _, err := v.Verify(proof[:len(proof)-1]); !errors.Is(err, ErrDecodeProof) {
					t.Fatalf("truncated proof: got %v, want ErrDecodeProof", err)
				}
				extended := append(append([]byte{}, proof...), 0xFF)
				if _, err := v.Verify(extended); !errors.Is(err, ErrDecodeProof) {
					t.Fatalf("extended proof: got %v, want ErrDecodeProof", err)
				}
			})

			t.Run("invalid_proof_verify", func(t *testing.T) {
				v, err := New(fx.kind, programVK)
				if err != nil {
					t.Fatalf("New: %v", err)
				}
				defer v.Close()

				flipped := append([]byte{}, proof...)
				flipped[len(flipped)/2] ^= 0xFF
				if _, err := v.Verify(flipped); !errors.Is(err, ErrVerify) {
					t.Fatalf("flipped proof: got %v, want ErrVerify", err)
				}

				wrongVK := append([]byte{}, programVK...)
				wrongVK[0] ^= 0xFF
				vv, err := New(fx.kind, wrongVK)
				if err != nil {
					t.Fatalf("New with unexpected vk: %v", err)
				}
				defer vv.Close()
				if _, err := vv.Verify(proof); !errors.Is(err, ErrVerify) {
					t.Fatalf("unexpected vk: got %v, want ErrVerify", err)
				}
			})

		})
	}
}

func TestInvalidZkVMKind(t *testing.T) {
	if _, err := New(ZkVMKind(99), nil); !errors.Is(err, ErrBadKind) {
		t.Fatalf("got %v, want ErrBadKind", err)
	}
}

func TestNilVerifier(t *testing.T) {
	var v *Verifier
	if _, err := v.Kind(); !errors.Is(err, ErrNullPtr) {
		t.Fatalf("Kind on nil receiver: got %v, want ErrNullPtr", err)
	}
	if _, err := v.Verify(nil); !errors.Is(err, ErrNullPtr) {
		t.Fatalf("Verify on nil receiver: got %v, want ErrNullPtr", err)
	}
	v.Close()
}

func TestClosedVerifier(t *testing.T) {
	dir := filepath.Join(workspaceRoot(t), "crates", "verifier", fixtures[0].name, "tests", "fixtures")
	v, err := New(fixtures[0].kind, mustRead(t, filepath.Join(dir, "program_vk.bin")))
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	v.Close()
	if _, err := v.Kind(); !errors.Is(err, ErrNullPtr) {
		t.Fatalf("Kind after Close: got %v, want ErrNullPtr", err)
	}
	v.Close()
}
