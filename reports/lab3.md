# rCore 第五章实验报告：进程管理

## 一、实验目的

本实验基于 rCore 第五章进程管理框架，学习进程创建、执行、
等待与回收机制，并实现 spawn、set_priority 等系统调用。
同时将上一章的时间与内存映射功能适配到第五章。

## 二、实验环境

- 操作系统：WSL2 Ubuntu 22.04.5
- 实验项目：LearningOS 2026a rCore
- 实验分支：ch5
- 目标架构：riscv64gc-unknown-none-elf
- 运行环境：QEMU

## 三、主要实现

### 1. sys_spawn

从用户地址空间读取程序路径，通过 get_app_data_by_name
找到对应 ELF 数据，再调用 TaskControlBlock::new 创建新进程。

新进程拥有独立地址空间、PID、内核栈和 TrapContext。

使用 Arc 和 Weak 建立父子进程关系，将子进程加入父进程
children 列表，并通过 add_task 加入就绪队列。

spawn 与 fork 不同：spawn 直接装载新程序，
不会先复制父进程的整个用户地址空间。

### 2. sys_set_priority

在 TaskControlBlockInner 中添加 priority 字段，
创建进程时设置默认优先级 16，fork 时继承父进程优先级。

sys_set_priority 对输入进行合法性检查：
优先级小于 2 时返回 -1，否则保存当前进程优先级并返回该值。

目前调度器仍采用 VecDeque 实现 FIFO 轮转调度。
本次实现完成了优先级保存与设置，但尚未实现完整的
Stride Scheduling，优先级不会实际改变 CPU 调度份额。

### 3. 时间系统调用

使用 get_time_us 获取微秒时间，
将其转换为 TimeVal 中的秒数与微秒数。

通过 translated_byte_buffer 写入用户地址空间，
能够处理 TimeVal 横跨两个虚拟页面的情况。

### 4. mmap 和 munmap

将第四章经过测试的匿名内存映射功能适配到第五章。

mmap 校验地址对齐、权限参数、地址范围及映射冲突，
通过 MemorySet 为用户进程分配和映射物理页。

munmap 在修改页表之前检查映射是否有效，
再解除页面映射并回收相应物理页帧。

第五章使用独立的进程地址空间，
fork 通过 MemorySet::from_existed_user 复制已有映射。

### 5. 进程管理

保留第五章已有的 fork、exec、waitpid 机制。

fork 创建子进程并复制用户地址空间；
exec 替换当前进程执行的用户程序；
waitpid 等待并回收已经退出的子进程。

## 四、测试与调试过程

使用以下命令进行编译和测试：

    make -C os build
    make -C ci-user test CHAPTER=5

测试进展：

- 初始测试：2/15
- 实现 sys_spawn 后：10/15
- 适配 mmap 和 munmap 后：14/15
- 实现 sys_set_priority 后：15/15

调试过程中解决的问题包括：

1. 未实现 spawn 导致子进程无法正常创建。
2. 移植公开方法后触发 missing_docs 编译错误。
3. 多余的 return 语句触发 unreachable_code 错误。
4. 未实现 set_priority 导致优先级测试失败。
5. 功能测试通过后仍需补齐实验报告文件。

最终本地功能测试结果：

    Test passed: 15/15

## 五、实验总结

本实验加深了对进程控制块、独立地址空间、
父子进程关系、进程创建与回收机制的理解。

spawn 通过直接加载 ELF 创建新进程，
而 fork 需要复制已有进程的地址空间。

通过适配第四章的内存管理功能，
进一步理解了虚拟内存与进程管理之间的联系。

后续可以继续实现真正的 Stride Scheduling，
使进程优先级影响 CPU 时间分配，并验证调度公平性。
