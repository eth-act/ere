// LLVM pass plugin for the ZisK static library: lowers memcpy, memmove, memset and memcmp/bcmp to
// ZisK's inline DMA precompiles, with the rules of ZisK's own toolchain (`src/llvm-patches/
// 0001-riscv-zisk-dma-lowering.patch` of github.com/0xPolygonHermez/rust, which does this in the
// RISC-V backend under the `zisk-dma` target feature).
//
// ZisK's transpiler fuses each marker sequence below into one `dma_xmemcpy`/`dma_xmemcmp`/
// `dma_xmemset` operation. A length up to 2047 (`addi`'s signed 12-bit immediate) is encoded in the
// instruction; a larger or a runtime length is passed in a register:
//
//   memcpy/memmove(dst, src, n)   csrs  0x813, src      ; addi x0, dst, n     | add x0, dst, n_reg
//   memcmp(a, b, n)               csrrs res, 0x814, b   ; addi x0, a, n       | add x0, a, n_reg
//   memset(dst, v, n)             csrsi 0x816, 2 ; addi x0, dst, n ; addi x0, dst, v
//                                 csrs  0x816, dst ; addi x0, n_reg, v   (runtime length)
//
// As in ZisK's toolchain: a constant length of 16 bytes or less keeps LLVM's inline load/store
// expansion (at most two XLEN-sized operations), every longer or runtime length is lowered, memset
// only with a constant fill byte, and volatile operations never. ZisK's memcpy handles overlapping
// ranges, so memmove uses it too.
//
// The pass runs at the end of the full-LTO optimization pipeline, so the optimizer has already
// removed every copy it could, and code generation follows directly. `link.sh` loads it with
// `ld.lld --load-pass-plugin` when the static library carries it. Loading it also caps LLVM's own
// inline expansion of these operations at two XLEN-sized operations, as ZisK's toolchain does,
// unless the link sets those limits itself.

#include "llvm/IR/IRBuilder.h"
#include "llvm/IR/InlineAsm.h"
#include "llvm/IR/InstIterator.h"
#include "llvm/IR/IntrinsicInst.h"
#include "llvm/IR/PassManager.h"
#include "llvm/Passes/PassBuilder.h"
#include "llvm/Plugins/PassPlugin.h"
#include "llvm/Support/CommandLine.h"

#include <optional>
#include <string>

using namespace llvm;

namespace {

constexpr unsigned MemcpyCsr = 0x813;
constexpr unsigned MemcmpCsr = 0x814;
constexpr unsigned MemsetCsr = 0x816;

// Constant lengths up to this many bytes keep the inline load/store expansion.
constexpr uint64_t MaxInlineBytes = 16;
// The largest length `addi` can encode.
constexpr uint64_t MaxImmBytes = 2047;

// How a length reaches the marker sequence: as an immediate, or in a register.
struct Length {
  std::optional<uint64_t> Imm;
  Value *Reg = nullptr;
};

// The length of an operation to lower, or nothing if it keeps the inline expansion.
std::optional<Length> dmaLength(Value *N, IRBuilder<> &B) {
  if (const auto *C = dyn_cast<ConstantInt>(N)) {
    if (C->getBitWidth() > 64 || C->getZExtValue() <= MaxInlineBytes)
      return std::nullopt;
    if (C->getZExtValue() <= MaxImmBytes)
      return Length{C->getZExtValue(), nullptr};
  }
  return Length{std::nullopt, B.CreateZExtOrTrunc(N, B.getInt64Ty())};
}

std::string hex(unsigned V) {
  return "0x" + utohexstr(V, /*LowerCase=*/true);
}

// `addi x0, $Base, <imm>` or `add x0, $Base, $Len`.
std::string countInstr(const Length &L, const char *Base, const char *Len) {
  if (L.Imm)
    return std::string("addi x0, ") + Base + ", " + std::to_string(*L.Imm);
  return std::string("add x0, ") + Base + ", " + Len;
}

struct ZiskDmaPass : PassInfoMixin<ZiskDmaPass> {
  PreservedAnalyses run(Module &M, ModuleAnalysisManager &) {
    LLVMContext &Ctx = M.getContext();
    Type *Ptr = PointerType::get(Ctx, 0);
    Type *Void = Type::getVoidTy(Ctx);
    Type *I64 = Type::getInt64Ty(Ctx);

    SmallVector<Instruction *, 64> Lowered;
    for (Function &F : M) {
      for (Instruction &I : instructions(F)) {
        IRBuilder<> B(&I);
        if (auto *Copy = dyn_cast<MemTransferInst>(&I)) {
          if (Copy->isVolatile() || Copy->getDestAddressSpace() != 0 ||
              Copy->getSourceAddressSpace() != 0)
            continue;
          auto L = dmaLength(Copy->getLength(), B);
          if (!L)
            continue;
          std::string Asm = "csrs " + hex(MemcpyCsr) + ", $1\n\t" + countInstr(*L, "$0", "$2");
          SmallVector<Type *, 3> Types = {Ptr, Ptr};
          SmallVector<Value *, 3> Args = {Copy->getRawDest(), Copy->getRawSource()};
          if (L->Reg) {
            Types.push_back(I64);
            Args.push_back(L->Reg);
          }
          B.CreateCall(InlineAsm::get(FunctionType::get(Void, Types, false), Asm,
                                      L->Reg ? "r,r,r,~{memory}" : "r,r,~{memory}",
                                      /*hasSideEffects=*/true),
                       Args);
          Lowered.push_back(&I);
        } else if (auto *Set = dyn_cast<MemSetInst>(&I)) {
          const auto *Byte = dyn_cast<ConstantInt>(Set->getValue());
          if (!Byte || Set->isVolatile() || Set->getDestAddressSpace() != 0)
            continue;
          auto L = dmaLength(Set->getLength(), B);
          if (!L)
            continue;
          std::string V = std::to_string(Byte->getZExtValue() & 0xff);
          if (L->Imm) {
            B.CreateCall(InlineAsm::get(FunctionType::get(Void, {Ptr}, false),
                                        "csrsi " + hex(MemsetCsr) + ", 2\n\taddi x0, $0, " +
                                            std::to_string(*L->Imm) + "\n\taddi x0, $0, " + V,
                                        "r,~{memory}", /*hasSideEffects=*/true),
                         {Set->getRawDest()});
          } else {
            B.CreateCall(InlineAsm::get(FunctionType::get(Void, {Ptr, I64}, false),
                                        "csrs " + hex(MemsetCsr) + ", $0\n\taddi x0, $1, " + V,
                                        "r,r,~{memory}", /*hasSideEffects=*/true),
                         {Set->getRawDest(), L->Reg});
          }
          Lowered.push_back(&I);
        } else if (auto *Call = dyn_cast<CallInst>(&I)) {
          const Function *Callee = Call->getCalledFunction();
          if (!Callee || Call->arg_size() != 3 || !Call->getType()->isIntegerTy() ||
              (Callee->getName() != "memcmp" && Callee->getName() != "bcmp"))
            continue;
          auto L = dmaLength(Call->getArgOperand(2), B);
          if (!L)
            continue;
          std::string Asm =
              "csrrs $0, " + hex(MemcmpCsr) + ", $2\n\t" + countInstr(*L, "$1", "$3");
          SmallVector<Type *, 3> Types = {Ptr, Ptr};
          SmallVector<Value *, 3> Args = {Call->getArgOperand(0), Call->getArgOperand(1)};
          if (L->Reg) {
            Types.push_back(I64);
            Args.push_back(L->Reg);
          }
          Value *Diff = B.CreateCall(
              InlineAsm::get(FunctionType::get(I64, Types, false), Asm,
                             L->Reg ? "=r,r,r,r,~{memory}" : "=r,r,r,~{memory}",
                             /*hasSideEffects=*/true),
              Args);
          Call->replaceAllUsesWith(B.CreateTrunc(Diff, Call->getType()));
          Lowered.push_back(&I);
        }
      }
    }
    for (Instruction *I : Lowered)
      I->eraseFromParent();
    return Lowered.empty() ? PreservedAnalyses::all() : PreservedAnalyses::none();
  }
};

// Sets an LLVM option to `Value`, unless the command line already set it.
void setDefaultOption(StringRef Name, StringRef Value) {
  auto &Options = cl::getRegisteredOptions();
  auto It = Options.find(Name);
  if (It != Options.end() && It->second->getNumOccurrences() == 0)
    It->second->addOccurrence(0, Name, Value);
}

} // namespace

extern "C" LLVM_ATTRIBUTE_WEAK PassPluginLibraryInfo llvmGetPassPluginInfo() {
  return {LLVM_PLUGIN_API_VERSION, "zisk-dma", "0.2", [](PassBuilder &PB) {
            // What LLVM does not lower here it expands inline only up to two XLEN-sized
            // operations, as ZisK's toolchain sets for its `zisk-dma` target.
            for (StringRef Name :
                 {"max-store-memcpy", "max-store-memmove", "max-store-memset",
                  "max-loads-per-memcmp"})
              setDefaultOption(Name, "2");
            PB.registerFullLinkTimeOptimizationLastEPCallback(
                [](ModulePassManager &MPM, OptimizationLevel) { MPM.addPass(ZiskDmaPass()); });
          }};
}
