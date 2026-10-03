<p align="center">
  <img src="huiji/assets/logo/huiji-logo.png" alt="灰迹" height="128">
</p>

<h1 align="center">灰迹 Huiji</h1>

<p align="center"><strong>把一整句拼音敲完，直接给你一整句译文。</strong></p>

<p align="center">
  <img src="https://img.shields.io/badge/license-GPL--3.0--or--later-blue" alt="License: GPL-3.0-or-later">
  <img src="https://img.shields.io/badge/Windows-10%2F11-blue" alt="Windows 10/11">
  <a href="https://github.com/noii-y/huiji/stargazers"><img src="https://img.shields.io/github/stars/noii-y/huiji?label=Stars" alt="GitHub Stars"></a>
</p>

灰迹是一款 Windows 拼音输入法，从开源输入法 [青简](https://github.com/qingjian-team/qingjian) 长出来。

青简会在每个候选词旁边标一个外文译词，但它只到「词」这一层。灰迹想再往前走一步：你照平常那样把一整句拼音连续敲完、不用停下来逐个选词，候选窗顶部就给出整句英文；手滑敲错一两个字母，也尽量把你真正想要的那个词排到前面。

## 两个主打能力

**端侧整句翻译，全程离线**

连续敲 `nijintianwanshangyoukongma`，不用逐词选，候选窗顶部直接出现 `Are you free tonight?`。翻译模型和运行时都在本机，不联网、不花钱，敲的内容不出这台电脑。在 100 句测试集上，译文延迟中位数 50 毫秒，多数句子通顺可用。模型是一个约 81 MB 的中英 Marian，已经随安装包一起装好。

**手误纠错，少堆规则**

默认开启「中文优先」后，声母缩写（比如敲 `kj` 想要「看见」）的首选命中率从 66.2% 提到 77.5%。只错一个字母的干净串，青简原本就能纠正大部分，灰迹没有在这上面硬堆手写规则——再往上的空间主要在个性化学习和上下文模型，规则表越加越容易误伤。

## 安装

到 [Releases](https://github.com/noii-y/huiji/releases) 下载最新的 `huiji-<版本>-windows-x86_64-setup.exe`，双击安装，按提示注销或重启一次即可。安装包约 124 MB，翻译模型和运行时都打在里面，装完离线就能用，不必再单独下载。

想自己从源码编译，见 [apps/windows/README.md](apps/windows/README.md)。

## 和青简的关系

灰迹基于青简（GPL-3.0-or-later）开发，保留了它本地优先、输入不被翻译拖慢的架构，也把完整提交历史带了过来、随时可以溯源。青简的作者和社区做了最难的地基工作，灰迹在此致谢。按 GPL 的要求，灰迹同样以 GPL-3.0-or-later 开源。随包数据各有来源与许可，见 [docs/design/landscape.md](docs/design/landscape.md)。

## 数据与隐私

拼音转换、词库查询、翻译和输入习惯学习都在本机完成，不需要账号。云联想和云端翻译默认关闭，只有你自己填入 AI 服务商的 key 之后才会发请求，而且请求直接发给服务商、不经过灰迹。

## 当前状态

灰迹目前聚焦 Windows，端侧整句翻译已经在真机闭环。云端整句翻译的链路也已经就绪，填入 OpenAI 兼容 key（DeepSeek 等）就能作为长难句的兜底。macOS、Linux 的外壳沿用青简代码，但灰迹还没在这两个平台上专门打磨。

接下来打算补：流行语和专名的本地术语表、云端兜底、个性化纠错。一步步的进展写在 [开发日志](huiji/开发日志.md) 里。

## 许可

[GPL-3.0-or-later](LICENSE)。
