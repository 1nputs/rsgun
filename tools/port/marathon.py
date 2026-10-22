"""An input script that plays through the whole game without skill: player 1 coins up and starts,
then fires all the time (cycling the weapons) while weaving across the screen, and puts in a coin
now and then and presses start often, so each game over is continued. Bosses that are not beaten
leave when their time runs out, so the game goes on to the end. For evidence of every stage's code
(tools/port/check.sh's evidence runs), not for scoring.

    python3 tools/port/marathon.py [--frames N] > out/evidence/marathon-input.txt

With --stage4 the same script takes stage 4 (STG03) at STG01's end instead of stage 2: the
controls let go from 65500 and down pressed once at 66900, while the choice is open (it opens
about 66800 and takes the cursor's place after 300 frames or at a button: the script's own from
67000):
    python3 tools/port/marathon.py --frames 230000 --stage4 > out/evidence/route4-input.txt
(its run starts from the marathon's f060000 state; the plain script is --frames 230000 too).
"""
import sys

START = 1500  # the attract mode is showing by then (frames count from power on)
COIN_EVERY = 600
START_EVERY = 90  # the continue countdown is short, and buttons may hurry it
WEAPONS = ["a", "b", "c", "ab", "ac", "bc", "a", "abc"]
WEAVE = ["left", "", "right", "", "right", "up", "left", "down"]


def main():
    frames = 150_000
    args = sys.argv[1:]
    stage4 = "--stage4" in args
    args = [a for a in args if a != "--stage4"]
    if args[:1] == ["--frames"]:
        frames = int(args[1])
    events = {}  # frame -> player 1's controls from then on

    def hold(f, what):
        events[f] = what

    hold(START, "coin")
    hold(START + 4, "")
    hold(START + 100, "start")
    hold(START + 104, "")
    f = START + 300
    next_coin = f + COIN_EVERY
    step = 0
    while f < frames:
        # a coin now and then, and start often (while playing they do nothing)
        if f >= next_coin:
            hold(f, "coin")
            hold(f + 4, "")
            f += 10
            next_coin += COIN_EVERY
            continue
        weapon = WEAPONS[(step // 3) % len(WEAPONS)]
        move = WEAVE[step % len(WEAVE)]
        controls = f"{move} {weapon}".strip()
        hold(f, controls)
        if step % (START_EVERY // 45) == 0:
            hold(f + 20, "start")
            hold(f + 24, controls)
        step += 1
        f += 45
    if stage4:
        events = {f: c for f, c in events.items() if not 65500 <= f < 67000}
        events.update({65500: "", 66900: "down", 66904: ""})
    print("# Through the whole game without skill: made by tools/port/marathon.py")
    for frame in sorted(events):
        print(f"{frame} {events[frame]}".rstrip())


if __name__ == "__main__":
    main()
