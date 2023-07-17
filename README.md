# ZUMA AI

This repo contains the code for an AI that plays Zuma. I started this project because I like zuma and it appears no other similar AI had been made. It was also a way for me to increase my skills in Rust and reverse engineering.

It only supports the steam version of Zuma Deluxe, running on linux under wine. Support for other platforms is not planned.

## Brief overview
This AI is composed of two main parts:
- The memory reading part
- The decision part

### Memory reading
In order to extract information from the game, the AI reads the values for the ball's colors and positions directly in the game's memory.
I chose this route instead of computer vision for multiple reasons:
- Reliablility
- Speed and frequency of the reads
- I wanted to increase my skill in reverse engineering

The logic for the memory reading is contained in the `mem_reader.rs`.

### Making decisions
After having retrieved the positions and colors of the balls, the AI can make a decision about where to shoot. It currently only tries to shoot the biggest group of balls that matches the color of the one that is in the frog's mouth.

This AI does not have any machine learning, it does not improve on its own. Its decisions come from a set of rules and logic defined by the programmer.

## Drawbacks
The AI lacks many things, some of which are listed here:
### Awareness of the balls that are in flight
Because of this the AI sometimes shoots twice in the same spot, popping a group of balls and immediately replacing it by a ball of the same color.

This is especially unsatisfying when it prevents a combo from continuing.

However, it does attempt to compensate for the balls that land during flight of the ball it is about to shoot; and that might push the targeted ball
### Knowledge of powerups that the balls contain
This means that the AI will treat balls that have powerups like any other ball.
### Knowledge of the bonuses that sometimes spawn on the map
When the AI collects a bonus, it is purely accidental, generally due to one of problems mentionned above
### Balls going backwards
The AI is able to predict future position of the balls based on their forward speed, or backwards speed when it is caused by a powerup or a level change.

However, for balls going backwars when balls of the same color are seperated by a gap, it currently doesn't know how to compensate and shoots as if the balls were stationary
