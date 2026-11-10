# rCore 实验报告 Lab 1：多道程序与分时多任务（ch3）

## 一、实验目标

本实验基于 LearningOS rCore 第三章 `ch3` 分支，在已能运行多道用户程序的内核中实现 `sys_trace` 系统调用（调用号 410），并为每个任务分别统计系统调用次数。通过第三章官方 Checker 验证时间、休眠和系统调用追踪功能。

## 二、实验环境

- 系统：Windows WSL2，Ubuntu 22.04.5 LTS（x86_64）
- 项目路径：`/mnt/d/rust/2026a-rcore-nie727904-creator`
- Rust：`rustc 1.80.0-nightly`（nightly-2024-05-02）
- RISC-V 编译目标：`riscv64gc-unknown-none-elf`
- QEMU：8.2.2，RISC-V `virt` 机器
- 固件：仓库内的 `bootloader/rustsbi-qemu.bin`
- 测试命令：`make -C ci-user test CHAPTER=3`

## 三、设计与实现

### 3.1 系统调用分发及统计时机

在 `os/src/syscall/mod.rs` 的 `syscall()` 函数入口调用 `crate::task::record_syscall(syscall_id)`，随后再根据调用号分发到具体系统调用处理函数。这样每次用户态 `ecall` 进入分发函数时，都会先增加当前任务相应调用号的计数。尤其在 `sys_trace(2, 410, 0)` 查询 `sys_trace` 自身调用次数时，返回值包含当前这一次查询操作。

### 3.2 按任务保存调用次数

在 `os/src/task/task.rs` 的 `TaskControlBlock` 中新增 `syscall_counts` 字段，类型为 `[usize; MAX_SYSCALL_NUM]`。本次实现设置 `MAX_SYSCALL_NUM = 500`，覆盖本章使用的系统调用号，包括 `write(64)`、`exit(93)`、`yield(124)`、`gettimeofday(169)` 和 `trace(410)`。

在 `os/src/task/mod.rs` 初始化任务数组时将各计数数组清零。新增 `record_syscall()` 和 `get_syscall_count()`，通过 `TASK_MANAGER.inner.exclusive_access()` 获取当前任务，分别增加和读取计数。系统调用号超过数组范围时，不执行越界访问；查询返回 `None`，由 `sys_trace()` 转换为 `-1`。

每个任务控制块具有独立的数组，因此任务调度切换不会混淆不同任务的统计数据。访问完成后局部独占借用结束，不会跨越任务切换调用持有借用。

### 3.3 实现 sys_trace

在 `os/src/syscall/process.rs` 中将原先固定返回 `-1` 的占位函数改为按 `trace_request` 分类处理：

| 请求值 | 功能 | 返回值 |
| --- | --- | --- |
| `0` | 将 `id` 视为用户字节地址，读取一个 `u8` | 读出的无符号字节值 |
| `1` | 将 `id` 视为用户字节地址，写入 `data` 的最低 8 位 | `0` |
| `2` | 将 `id` 视为系统调用号，查询当前任务的累计调用次数 | 调用次数，越界时 `-1` |
| 其他 | 无效请求 | `-1` |

对于读写请求，本次实现对空地址返回 `-1`，并使用 Rust `unsafe` 指针操作完成字节访问。由于第三章尚未实现后续章节的用户虚拟地址空间与页表校验，本实现没有完整的用户指针合法性检查，不能直接作为具有内存隔离的通用内核实现。

### 3.4 与 Trap 和任务切换的关系

用户程序执行 `ecall` 后，处理器进入内核的 Trap 处理流程。`os/src/trap/mod.rs` 的 `trap_handler()` 在 `UserEnvCall` 分支中将 `sepc` 增加 4，避免返回后重复执行同一条 `ecall`，然后取 `a7` 寄存器中的系统调用号和 `a0`～`a2` 中的参数，调用 `syscall()` 并将返回值写回 `a0`。

定时器中断通过设置下次触发时间并调用 `suspend_current_and_run_next()` 切换任务；`sys_yield()` 也会主动让出处理器。任务切换保存和恢复任务上下文，而系统调用统计数据保留在每个任务的控制块中。

## 四、测试过程与结果

### 4.1 修改前基线

在未实现 `sys_trace` 的原始代码上执行 `make -C ci-user test CHAPTER=3`，基础输出匹配为 **5/7**。`ch3_trace` 在查询 `SYSCALL_GETTIMEOFDAY` 的次数时断言失败，检查器未找到 `string from task trace test` 和 `Test trace OK...` 两条输出。

### 4.2 修改后编译

补丁应用完成后，通过 `make -C os build` 完成用户程序和 RISC-V 内核的 Release 编译，无编译错误。

### 4.3 修改后 Checker 测试

再次执行：

```bash
make -C ci-user test CHAPTER=3
```

检查器输出的七项结果全部为 `[PASS]`，包括 `get_time`、`sleep`、`sleep1`、时间信息以及 `sys_trace` 相关输出：

```text
[PASS] found <string from task trace test>
[PASS] found <Test trace OK62735!>
Test passed62735: 7/7
```

随后检查器报告：

```text
Report for lab1 needed. Add your report to reports/lab1.pdf or reports/lab1.md
```

这说明**七项功能测试已经通过**，但在编写本报告之前，本地 Checker 因缺少必需的报告文件而以错误码退出。报告保存后还需重新运行 Checker，确认整个检查流程最终成功退出。

## 五、实验总结与思考

本次实验理解了用户态 `ecall` 到内核系统调用分发的路径、任务控制块保存任务私有状态的方式，以及通过定时器中断和主动 `yield` 完成分时调度的基本机制。

实现中最关键的细节是**先计数、后分发**：否则查询 `sys_trace` 本身次数时会少统计当前调用。此外，系统调用次数必须按任务分别存储，不能简单使用一个全局计数数组。

当前第三章内核可以直接用指针访问用户程序提供的地址，但这存在内存安全风险。后续学习地址空间和页表后，需要通过地址转换及合法性检查安全地访问用户内存。

**最终验证状态：** 本报告依据已完成的 `7/7` 功能测试及实际源码修改编写；报告加入仓库后仍须完成一次完整 Checker 复测，并通过 Git 提交与远程 CI 验证。
