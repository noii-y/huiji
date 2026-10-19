**English** | [中文](README.md)

# huiji (灰迹)

A Windows pinyin input method that shows you the English meaning of every candidate while you type, and can translate a whole pinyin sentence into English offline. huiji is a fork of [qingjian-team/qingjian](https://github.com/qingjian-team/qingjian), keeps the full commit history, and stays GPL-3.0-or-later.

## Why

Most of us type Chinese all day and never look at the candidates again. huiji puts the vocabulary where your fingers already are: each candidate carries its English meaning and part of speech, and words you haven't learned yet show up in orange until they stop being new and fade to gray.

The part people don't expect is the sentence translation. Type `nijintianwanshangyoukongma` and the candidate window offers `Are you free tonight?`. The model is bundled inside the installer, so it works with the network cable unplugged.

## Features

- **Translation next to candidates.** English meaning and part of speech beside every pinyin candidate. This is the signature qingjian feature, and huiji keeps it.
- **Whole-sentence offline translation.** Type a complete pinyin sentence and get an English sentence in the candidate list. The model ships inside the installer; no network call involved.
- **New-word highlighting.** Unfamiliar words render in orange, then settle to gray as you keep meeting them.
- **English and Japanese.** Japanese candidates come with kana readings.
- **Gaming-friendly.** Yields to fullscreen games, and the key layout matches Microsoft Pinyin, so switching between IMEs doesn't fight your muscle memory.
- **Optional cloud suggestions.** Off by default. If you want better long-tail suggestions, point it at any OpenAI-compatible endpoint with your own key (a DeepSeek key runs roughly 1–3 CNY/month 【estimated】).

## Demo

[▶ 11-second demo, recorded offline](docs/huiji-demo.mp4)

## Install (Windows x64)

Download the latest installer from [Releases](https://github.com/noii-y/huiji/releases). Current version: v0.1.8 (2026-10-08). The installer is self-contained and includes the translation model.

## Offline & Privacy

By default huiji makes no network requests. Pinyin conversion and translation happen locally; the model lives inside the installer. Cloud suggestions are strictly opt-in — nothing is sent anywhere unless you enable the feature and provide your own API key.

## Relationship to Qingjian

huiji derives from [qingjian-team/qingjian](https://github.com/qingjian-team/qingjian) and inherits its GPL-3.0-or-later license, with the full commit history preserved. The Chinese README has a dedicated "与青简的关系" section that spells out what was kept and what was added. If you find qingjian interesting, star it — huiji would not exist without it.

## Roadmap

Feature direction comes from issues and user feedback — if something is missing, open an issue rather than assuming someone else will. Dependency upkeep runs through Dependabot PRs.

## License

GPL-3.0-or-later, the same as the upstream qingjian project.
