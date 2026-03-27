class Color:
    def __init__(self, value):
        self.value = value

    @staticmethod
    def parse(value):
        return Color(value)

    def __repr__(self):
        return f"<Color {self.value}>"
