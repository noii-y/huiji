# -*- coding: utf-8 -*-
"""
把下载的原始语料清洗成「每行一段自然中文」的文本，交给 qingjian-cli --eval-text。
六类：外卖口语 / 酒店口语 / 电商评论 / 日常通用(Tatoeba) / 新闻报纸(人民日报) / 古典文学(红楼梦)。
固定随机种子，每类采样同样多的段落，保证可复现、可回归。
"""
import os, csv, re, io, zipfile, random, sys
from opencc import OpenCC

BASE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(BASE, 'corpus')
os.makedirs(OUT, exist_ok=True)
random.seed(42)

cc = OpenCC('t2s')
SAMPLE_PER_KIND = 5000   # 每类最多取多少段（行）

def clean_line(s):
    """统一清洗：繁转简、去 BOM/全角空格/HTML 空格、合并多余空白。"""
    s = cc.convert(s)
    s = s.replace('\ufeff', '').replace('　', ' ').replace('\u3000', ' ')
    s = s.replace('&nbsp;', ' ')
    s = re.sub(r'\s+', ' ', s).strip()
    return s

def load_csv(path):
    """label,review 或 cat,label,review：取 review 列。"""
    rows = []
    with open(path, 'r', encoding='utf-8', errors='ignore') as f:
        reader = csv.reader(f)
        header = next(reader)
        try:
            ri = header.index('review')
        except ValueError:
            ri = len(header) - 1
        for row in reader:
            if len(row) > ri:
                t = clean_line(row[ri])
                if len(t) >= 6:
                    rows.append(t)
    return rows

def load_tatoeba(path):
    rows = []
    with open(path, 'r', encoding='utf-8', errors='ignore') as f:
        for line in f:
            parts = line.rstrip('\n').split('\t')
            if len(parts) >= 3:
                t = clean_line(parts[2])
                # 只要含汉字、且不是纯符号
                if re.search(r'[\u4e00-\u9fff]', t) and len(re.findall(r'[\u4e00-\u9fff]', t)) >= 3:
                    rows.append(t)
    return rows

def load_people(zip_path):
    """人民日报 1998：每行 编号 词/词性 词/词性…… 去编号、去词性标记，拼回句子。"""
    rows = []
    z = zipfile.ZipFile(zip_path)
    for name in z.namelist():
        if not name.endswith('.txt') or name.startswith('__MACOSX'):
            continue
        text = z.read(name).decode('utf-8', errors='ignore')
        for line in text.splitlines():
            line = re.sub(r'^\d{8}-\d{2}-\d{3}-\d{3}', '', line)
            # 去掉每个词后的 /词性
            line = re.sub(r'/[A-Za-z]+', '', line)
            # 命名实体合并标注：[ 词 词 ]nt，去掉左括号和 ]词性、右括号
            line = line.replace('[', '')
            line = re.sub(r'\][A-Za-z]+', '', line)
            line = line.replace(']', '')
            line = re.sub(r'\s+', '', line)
            line = clean_line(line)
            if len(re.findall(r'[\u4e00-\u9fff]', line)) >= 6:
                rows.append(line)
    return rows

def load_hlm(folder):
    """红楼梦 markdown：去脂批代码块、回目标题、markdown 符号。"""
    rows = []
    for fn in sorted(os.listdir(folder)):
        if not fn.endswith('.md'):
            continue
        text = open(os.path.join(folder, fn), 'r', encoding='utf-8', errors='ignore').read()
        text = re.sub(r'```.*?```', '', text, flags=re.S)          # 批注代码块
        text = re.sub(r'【第[^】]*】[^\n]*', '', text)             # 回目标题
        text = re.sub(r'^#{1,6}\s*', '', text, flags=re.M)
        text = re.sub(r'^[-=\*]{3,}\s*$', '', text, flags=re.M)
        for line in text.splitlines():
            line = clean_line(line)
            if len(re.findall(r'[\u4e00-\u9fff]', line)) >= 6:
                rows.append(line)
    return rows

def sample(rows, n):
    random.shuffle(rows)
    return rows[:n]

def write(name, rows):
    path = os.path.join(OUT, name)
    seen = set()
    uniq = []
    for r in rows:
        if r not in seen:
            seen.add(r)
            uniq.append(r)
    with open(path, 'w', encoding='utf-8', newline='\n') as f:
        for r in uniq:
            f.write(r + '\n')
    return len(uniq)

jobs = []
jobs.append(('s01_外卖口语.txt', load_csv(os.path.join(BASE, 'waimai_10k.csv'))))
jobs.append(('s02_酒店口语.txt', load_csv(os.path.join(BASE, 'ChnSentiCorp_htl_all.csv'))))
jobs.append(('s03_电商评论.txt', load_csv(os.path.join(BASE, 'online_shopping.csv'))))
jobs.append(('s04_日常通用.txt', load_tatoeba(os.path.join(BASE, 'cmn_sentences.tsv'))))
jobs.append(('s05_新闻报纸.txt', load_people(os.path.join(BASE, 'people1998.zip'))))
jobs.append(('s06_古典文学.txt', load_hlm(os.path.join(BASE, 'hlm'))))

for name, rows in jobs:
    n = write(name, sample(rows, SAMPLE_PER_KIND))
    print(f'{name}: 清洗后 {len(rows)} 段, 采样去重后 {n} 段')
