"""全屏兼容测试窗口：造一个盖满屏幕、无标题栏的顶层窗口（形态对齐独占 / 无边框全屏游戏）。

用法：python fullscreen_test.py
- 窗口全屏后点输入框打字：游戏模式生效时，灰迹不弹自绘候选窗，按键直通，输入框里直接出字母。
- 按 Esc 退出；退出后回到普通窗口打字，候选窗应恢复。
"""

import tkinter as tk

root = tk.Tk()
root.title("全屏测试")
root.attributes("-fullscreen", True)
root.configure(bg="#202020")

label = tk.Label(
    root,
    text="全屏窗口（模拟全屏游戏）\n"
    "点下方输入框打字：不弹候选窗、按键直接出字母，说明游戏模式生效\n"
    "按 Esc 退出",
    fg="white",
    bg="#202020",
    font=("Microsoft YaHei", 16),
)
label.pack(pady=60)

entry = tk.Entry(root, font=("Microsoft YaHei", 18), width=40)
entry.pack(pady=20)
entry.focus_set()


def close(_event=None):
    root.destroy()


root.bind("<Escape>", close)
root.mainloop()
