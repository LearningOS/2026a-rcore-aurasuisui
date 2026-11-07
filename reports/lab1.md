# 实验一：多道程序与分时多任务 —— 系统调用跟踪（sys_trace）

## 一、功能实现概述（200 字以内）

本实验在 ch3 分时多任务内核中实现了 `sys_trace` 系统调用（syscall id 410），提供三类子功能：
`trace_request = 0` 时由内核读取当前任务用户地址空间中的一个字节并返回其值，地址非法返回 -1；
`trace_request = 1` 时由内核把 `data` 低 8 位写入指定用户地址，成功返回 0，地址非法返回 -1；
`trace_request = 2` 时查询当前任务对编号 `id` 的系统调用已发生的次数。
计数在系统调用入口、分发到具体处理函数之前完成，因此查询调用本身也会被计入。
每个任务控制块新增 `syscall_times: [u32; 500]` 数组做桶计数，随任务切换自然隔离。

## 二、问答题

### 1. 正确进入 U 态后程序的特征

进入 U 态后，程序运行在最低特权级，**不能执行任何 S 态与 M 态指令**，也不能访问 S 态寄存器。
运行框架提供的 `ch2b_bad_instructions`、`ch2b_bad_register`、`ch2b_bad_address` 三个测例时，
内核打印类似：

```
[kernel] IllegalInstruction in application, kernel killed it.
[kernel] PageFault in application, bad addr = 0x..., bad instruction = 0x..., kernel killed it.
```

即：执行特权指令（如 `csrw sstatus`、`mret`）触发非法指令异常，访问未映射的 S 态地址触发缺页异常，
均由内核终止该应用，说明 U/S 态隔离已经生效。
本次实验使用 **RustSBI 0.4.1**（随课程仓库 `bootloader/rustsbi-qemu.bin` 提供）。

### 2. `__alltraps` 与 `__restore`

`__alltraps` 是用户态陷入内核的唯一入口：它先把 `sp` 与 `sscratch` 交换，取得内核栈，
再在内核栈上保存 34 个通用寄存器、`sstatus`、`sepc` 与 `sscratch`，组成 `TrapContext`，
随后调用 `trap_handler` 并在返回后进入 `__restore`。

`__restore` 负责恢复现场并返回用户态，它有**两种使用情景**：一是内核首次运行应用时，
手工构造一个 `TrapContext` 后调用它进入 U 态；二是每次 `trap_handler` 处理完毕后返回用户态。
因此进入 `__restore` 时，`a0` 指向当前任务的 `TrapContext`。

对 `L43-L48` 的寄存器特殊处理：

- `ld t0, 32*8(sp)` 取出 **`sstatus`**：恢复 U 态相关的中断使能位与 `SPP`（返回后所处的特权级）。
- `ld t1, 33*8(sp)` 取出 **`sepc`**：指定 `sret` 之后从这里继续执行用户代码。
- `ld t2, 2*8(sp)` 取出用户态的 **`sp`**，其中保存的是用户栈指针。

`L50-L56` 跳过 `x2`（`sp`）与 `x4`（`tp`）：`sp` 在返回前由 `csrrw sp, sscratch, sp` 单独交换恢复，
`tp` 用于线程指针、不参与用户态现场恢复，因此这两者不通过通用 `LOAD_GP` 宏加载。

`L60` 执行 `csrrw sp, sscratch, sp` 后：`sp` 变为用户栈指针，`sscratch` 保存内核栈指针，
这样下次从 U 态陷入时仍能通过 `sscratch` 找回内核栈。

`__restore` 中**发生状态切换的指令是 `sret`**：它从 `sepc` 取出返回地址、依据 `sstatus.SPP`
把特权级从 S 态降为 U 态，并恢复 `sstatus.SIE`，因此执行后即回到用户态。
而 `L13` 处的 `csrrw sp, sscratch, sp` 是 `__alltraps` 的第一条指令，执行后 `sp` 变成内核栈指针、
`sscratch` 保存用户栈指针；**从 U 态进入 S 态是由 `ecall`（或中断/异常）触发的 `__alltraps` 入口**，
而不是某条显式的“切换指令”。

## 三、荣誉准则

本人郑重声明：本实验的代码与报告由本人独立完成，未抄袭他人代码或报告，未使用任何规避评测的手段；
实验过程中参考的公开资料已在报告中注明。本人理解并愿意遵守课程的学术规范，
如违反上述承诺，愿意接受成绩作废等处理。

## 四、对实验设计的反馈

sys_trace 用一个系统调用同时覆盖“内核读写用户内存”与“系统调用计数”两件事，
把地址空间的安全检查（ch4）自然地引了出来，设计紧凑；桶计数数组的容量提示也很清楚。
