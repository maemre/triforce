import sys, pickle, json

def mkcoord(x, y):
    return y, 2 * x + y

covers = pickle.load(open(sys.argv[1], 'rb'))

j = []
for cover in covers.keys():
    cover = sorted([mkcoord(x, y) for x, y in cover])
    j.append(json.dumps(cover, separators=(',', ':')))

j.sort()

for c in j:
    print(c)
