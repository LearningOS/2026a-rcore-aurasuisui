# 实验三：进程管理 —— spawn 系统调用与 stride 调度

## 一、功能实现概述（200 字以内）

本实验在 ch5 进程管理的基础上实现了两项功能。其一为 `sys_spawn`：根据用户传入的程序路径名找到内嵌
ELF 镜像，新建一个独立的地址空间与任务控制块（不像 `fork` 那样复制父进程地址空间），
把新进程挂到当前进程的 children 列表并放入就绪队列，返回子进程 pid；名字无效返回 -1。
其二为 stride 调度：为每个进程维护 `stride` 与 `pass`（`pass = BIG_STRIDE / priority`），
调度时总是选取 stride 最小的就绪进程，选中后让其 stride 加上自己的 pass，从而实现按优先级比例分配 CPU 时间。
另外实现 `sys_set_priority`，优先级小于 2 时返回 -1。

## 二、问答作业（stride 算法的反转现象）

### 1. 优先级 255 与 250 的例子

理论上下一次应该轮到 p1（stride 更小），**实际也会轮到 p1**。因为此时两者只差 5，
远小于 BigStride/2，比较关系没有发生“反转”；只有跨度超过 BigStride/2 时，溢出才会让“大的看起来更小”。

### 2. 为什么优先级 >= 2 能保证 STRIDE_MAX - STRIDE_MIN <= BigStride/2

设所有进程的 pass 最大为 P（P = BigStride/priority，优先级 >= 2 意味着 P <= BigStride/2）。
调度总是选最小 stride 并加上它的 pass，所以任意进程被选中后其 stride 变成“原最小值 + pass”，
至多为“原最小值 + BigStride/2”；而不被选中的进程 stride 仍不小于原最小值。
因此所有进程的 stride 始终落在长度为 BigStride/2 的区间内，
即 STRIDE_MAX - STRIDE_MIN <= BigStride/2。这也正是“优先级至少为 2”这一限制的由来。

### 3. 考虑溢出的 Stride 比较器

```rust
use core::cmp::Ordering;

struct Stride(u64);

impl PartialOrd for Stride {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.0 == other.0 {
            return Some(Ordering::Equal);
        }
        // 若两者之差超过半个 u64 范围，说明发生了回绕，大小关系应当反过来
        if self.0.wrapping_sub(other.0) < (1u64 << 63) {
            Some(Ordering::Less)
        } else {
            Some(Ordering::Greater)
        }
    }
}

impl PartialEq for Stride {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
```

## 三、荣誉准则

本人郑重声明：本实验的代码与报告由本人独立完成，未抄袭他人代码或报告，未使用任何规避评测的手段；
实验过程中参考的公开资料已在报告中注明。本人理解并愿意遵守课程的学术规范，
如违反上述承诺，愿意接受成绩作废等处理。

## 四、对实验设计的反馈

spawn 与 fork+exec 的对比很好地揭示了“地址空间复制”这一开销的本质；
stride 调度从公式推导到溢出比较器层层递进，比单纯实现轮转调度更能体会调度器设计中的数值陷阱。
