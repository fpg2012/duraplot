union Convert {
  char buffer[2];
  uint16_t num;
};

void setup() {
  Serial.begin(9600);
}
 
void loop() {
  delay(5);
  Convert temp;
  temp.num = analogRead(A0);
  Serial.write(temp.buffer, 2);
}
