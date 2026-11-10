# rCore 第六章实验报告：文件系统

## 一、实验目的

本实验基于 LearningOS 2026a rCore 第六章文件系统框架，
实现文件元数据查询、硬链接创建和解除链接操作。

通过实验理解文件描述符、inode、目录项、文件数据块
之间的关系，并掌握用户态系统调用与 easy-fs 文件系统
之间的交互过程。

## 二、实验环境

- 操作系统：WSL2 Ubuntu 22.04.5
- 项目：LearningOS 2026a rCore
- 实验分支：ch6
- 目标架构：riscv64gc-unknown-none-elf
- 运行环境：QEMU
- 文件系统：easy-fs

## 三、主要实现

### 1. 第五章功能迁移

第六章在第五章基础上增加了文件描述符表和文件系统支持。

为了保留第六章新增的数据结构，没有直接覆盖
task.rs 和 memory_set.rs，而是选择性迁移：

- sys_get_time
- sys_mmap 和 sys_munmap
- sys_spawn
- sys_set_priority

其中 sys_spawn 使用 open_file 和 read_all
从文件系统读取 ELF 文件，再创建新的进程。

### 2. sys_fstat

在 File trait 中添加 stat 接口，
由 OSInode 提供普通文件的元数据。

sys_fstat 根据文件描述符查找对应文件，
获取 Stat 数据，并通过用户地址转换函数
将结果复制到用户内存。

Stat 包括设备编号、inode 编号、文件类型
以及硬链接数量。

### 3. sys_linkat

通过 translated_str 读取用户提供的文件名，
调用文件系统的 link 操作。

link 不复制文件内容，而是在目录中创建新的目录项，
使新旧文件名指向同一个 inode。

如果源文件不存在或目标文件名已经存在，
则返回错误。

### 4. sys_unlinkat

通过 translated_str 读取文件名，
调用文件系统的 unlink 操作。

unlink 找到对应目录项后，将其清空，
使该文件名无法再次被查找到。

删除某个硬链接不会影响其他仍然存在的
目录项访问相同的 inode。

### 5. inode 编号与硬链接数量

在 easy-fs 中增加 link_info 方法，
通过目录项扫描统计指向同一个 inode 的链接数。

fstat 使用实际统计结果填充 ino 和 nlink，
而不再固定返回 nlink = 1。

在测试中验证：

- 创建文件后 nlink 为 1。
- 增加一个硬链接后 nlink 为 2。
- 增加三个硬链接后 nlink 为 4。
- 删除链接后 nlink 相应减少。
- 通过剩余硬链接可以继续读取原文件内容。

### 6. 目录项复用

修改 easy-fs 的 create 操作，
创建文件时优先寻找已经清空的目录项。

如果存在空闲目录项，则复用该位置；
否则向目录追加新的目录项。

这避免了反复创建和删除同名文件时，
目录项数量持续增长的问题。

需要说明的是，当前 unlink 实现主要解除目录项引用，
尚未完整实现最后一个硬链接删除后
inode 和数据块的自动回收。

## 四、编译与测试

使用以下命令编译：

    make -C os build

使用以下命令进行第六章功能测试：

    make -C ci-user test CHAPTER=6

调试过程中的测试结果：

- 初始状态：2/31
- 迁移第五章功能后：28/31
- 完成基础 fstat 后：29/31
- 完成 link、unlink 和链接数量统计后：31/31

最终测试输出：

    Test passed: 31/31

其中以下测试均成功：

- Test fstat OK
- Test link OK
- Test mass open/unlink OK

## 五、遇到的问题及解决方法

### 1. 第五章 spawn 无法直接迁移

第五章通过加载器获取 ELF 数据，
第六章则使用文件系统存储程序。

解决方法是使用 open_file 和 read_all
读取可执行文件，再调用 TaskControlBlock::new。

### 2. 文件元数据查询

原有 File trait 只有 read 和 write 等接口，
无法直接获取 Stat。

通过增加 stat 方法，
让文件系统对象向系统调用提供元数据。

### 3. 硬链接语义

最初的文件系统只支持创建独立 inode，
无法使多个文件名指向同一个文件。

通过新增目录项并复用原 inode 编号，
实现真正的硬链接关系。

### 4. 大量创建删除测试

如果每次创建文件都追加目录项，
目录可能持续增长。

通过重用删除后留下的空目录项，
使多次创建和删除操作能够通过测试。

## 六、实验总结

通过本实验，进一步理解了文件系统的分层结构，
掌握了系统调用、文件描述符、OSInode、
easy-fs Inode 和磁盘目录项之间的关系。

硬链接的核心是多个目录项引用同一 inode，
而不是创建多个独立文件。

本次实验完成了 fstat、linkat 和 unlinkat，
并通过第六章全部 31 项本地功能测试。

后续仍可完善 inode 与数据块的引用计数和回收，
提升文件系统在长期运行与重复创建删除场景下
的资源利用效率。
